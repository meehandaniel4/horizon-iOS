import Compression
import Foundation
import SwiftUI
import UIKit
import UniformTypeIdentifiers

// MARK: - Game Folders and Storage

enum GameFolder {
    static var documents: URL {
        FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first
            ?? URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent("Documents")
    }

    static var resources: URL { documents.appendingPathComponent("resources") }
    static var optionsFile: URL { documents.appendingPathComponent("options.txt") }

    static var hasGameFiles: Bool {
        let fm = FileManager.default
        let json = resources.appendingPathComponent("horizon-resources.json")
        if fm.fileExists(atPath: json.path) { return true }
        if fm.fileExists(atPath: resources.path) {
            let contents = (try? fm.contentsOfDirectory(atPath: resources.path)) ?? []
            if !contents.isEmpty { return true }
        }
        return false
    }

    static func readSensitivity() -> Double {
        guard let text = try? String(contentsOf: optionsFile, encoding: .utf8) else { return 0.5 }
        for line in text.components(separatedBy: .newlines) {
            if line.hasPrefix("mouseSensitivity:") {
                let parts = line.components(separatedBy: ":")
                if parts.count >= 2, let val = Double(parts[1].trimmingCharacters(in: .whitespaces)) {
                    return val
                }
            }
        }
        return 0.5
    }

    static func saveSensitivity(_ val: Double) {
        var lines: [String] = []
        var found = false
        if let text = try? String(contentsOf: optionsFile, encoding: .utf8) {
            for line in text.components(separatedBy: .newlines) {
                if line.hasPrefix("mouseSensitivity:") {
                    lines.append("mouseSensitivity:\(String(format: "%.4f", val))")
                    found = true
                } else if !line.isEmpty {
                    lines.append(line)
                }
            }
        }
        if !found {
            lines.append("mouseSensitivity:\(String(format: "%.4f", val))")
        }
        try? lines.joined(separator: "\n").write(to: optionsFile, atomically: true, encoding: .utf8)
    }
}

// MARK: - Unzip Helper

enum Unzip {
    static func resources(from zipURL: URL, into folder: URL) throws -> Int64 {
        let data = try Data(contentsOf: zipURL, options: .alwaysMapped)
        let fm = FileManager.default
        let resourcesFolder = folder.appendingPathComponent("resources")
        try fm.createDirectory(at: resourcesFolder, withIntermediateDirectories: true)
        var written: Int64 = 0
        try data.withUnsafeBytes { (raw: UnsafeRawBufferPointer) in
            func u16(_ at: Int) throws -> Int {
                guard at >= 0, at + 2 <= raw.count else { throw NSError(domain: "zip", code: 1, userInfo: [NSLocalizedDescriptionKey: "zip is cut short"]) }
                return Int(raw.loadUnaligned(fromByteOffset: at, as: UInt16.self).littleEndian)
            }
            func u32(_ at: Int) throws -> Int {
                guard at >= 0, at + 4 <= raw.count else { throw NSError(domain: "zip", code: 2, userInfo: [NSLocalizedDescriptionKey: "zip is cut short"]) }
                return Int(raw.loadUnaligned(fromByteOffset: at, as: UInt32.self).littleEndian)
            }
            var end = -1
            var at = raw.count - 22
            while at >= max(0, raw.count - 65_558) {
                if try u32(at) == 0x0605_4b50 { end = at; break }
                at -= 1
            }
            guard end >= 0 else { throw NSError(domain: "zip", code: 3, userInfo: [NSLocalizedDescriptionKey: "not a valid zip file"]) }
            let count = try u16(end + 10)
            var entry = try u32(end + 16)
            for _ in 0..<count {
                guard try u32(entry) == 0x0201_4b50 else { throw NSError(domain: "zip", code: 4, userInfo: [NSLocalizedDescriptionKey: "damaged zip directory"]) }
                let method = try u16(entry + 10)
                let packed = try u32(entry + 20)
                let size = try u32(entry + 24)
                let nameLength = try u16(entry + 28)
                let extra = try u16(entry + 30)
                let comment = try u16(entry + 32)
                let local = try u32(entry + 42)
                guard entry + 46 + nameLength <= raw.count,
                      let name = String(bytes: raw[(entry + 46)..<(entry + 46 + nameLength)], encoding: .utf8) else {
                    throw NSError(domain: "zip", code: 5, userInfo: [NSLocalizedDescriptionKey: "damaged zip entry name"])
                }
                entry += 46 + nameLength + extra + comment
                if name.contains("..") || name.hasPrefix("/") { continue }
                let to = name.hasPrefix("resources/") ? folder.appendingPathComponent(name) : resourcesFolder.appendingPathComponent(name)
                if name.hasSuffix("/") {
                    try fm.createDirectory(at: to, withIntermediateDirectories: true)
                    continue
                }
                try fm.createDirectory(at: to.deletingLastPathComponent(), withIntermediateDirectories: true)
                let header = local
                guard try u32(header) == 0x0403_4b50 else { throw NSError(domain: "zip", code: 6, userInfo: [NSLocalizedDescriptionKey: "damaged zip header"]) }
                let start = try header + 30 + u16(header + 26) + u16(header + 28)
                guard start + packed <= raw.count else { throw NSError(domain: "zip", code: 7, userInfo: [NSLocalizedDescriptionKey: "zip is cut short"]) }
                let source = UnsafeRawBufferPointer(rebasing: raw[start..<(start + packed)])
                var out = Data(count: size)
                switch method {
                case 0:
                    guard packed == size else { throw NSError(domain: "zip", code: 8, userInfo: [NSLocalizedDescriptionKey: "\(name) size mismatch"]) }
                    out = Data(source)
                case 8:
                    let n = out.withUnsafeMutableBytes { dst -> Int in
                        guard size > 0 else { return 0 }
                        return compression_decode_buffer(dst.bindMemory(to: UInt8.self).baseAddress!, size,
                                                         source.bindMemory(to: UInt8.self).baseAddress!, packed,
                                                         nil, COMPRESSION_ZLIB)
                    }
                    guard n == size else { throw NSError(domain: "zip", code: 9, userInfo: [NSLocalizedDescriptionKey: "\(name) decompression failed"]) }
                default:
                    throw NSError(domain: "zip", code: 10, userInfo: [NSLocalizedDescriptionKey: "unsupported compression method"])
                }
                try out.write(to: to)
                written += Int64(size)
            }
        }
        return written
    }
}

// MARK: - Resource Downloader

final class ResourceDownloader: NSObject, ObservableObject, URLSessionDownloadDelegate {
    @Published var isDownloading = false
    @Published var progress: Double = 0.0
    @Published var statusText: String = ""
    @Published var error: String? = nil

    private var session: URLSession?
    private var completion: (() -> Void)?

    func download(from url: URL, completion: @escaping () -> Void) {
        guard !isDownloading else { return }
        self.completion = completion
        self.error = nil
        self.progress = 0.0
        self.isDownloading = true
        self.statusText = "Подключение к серверу..."

        let config = URLSessionConfiguration.default
        session = URLSession(configuration: config, delegate: self, delegateQueue: .main)
        let task = session?.downloadTask(with: url)
        task?.resume()
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didWriteData bytesWritten: Int64, totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64) {
        if totalBytesExpectedToWrite > 0 {
            let p = Double(totalBytesWritten) / Double(totalBytesExpectedToWrite)
            self.progress = p
            let mbWritten = Double(totalBytesWritten) / 1024.0 / 1024.0
            let mbTotal = Double(totalBytesExpectedToWrite) / 1024.0 / 1024.0
            self.statusText = String(format: "Загрузка: %.1f / %.1f МБ (%.0f%%)", mbWritten, mbTotal, p * 100)
        } else {
            let mbWritten = Double(totalBytesWritten) / 1024.0 / 1024.0
            self.statusText = String(format: "Загрузка: %.1f МБ", mbWritten)
        }
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {
        self.statusText = "Распаковка ресурсов..."
        DispatchQueue.global(qos: .userInitiated).async {
            do {
                _ = try Unzip.resources(from: location, into: GameFolder.documents)
                DispatchQueue.main.async {
                    self.isDownloading = false
                    self.statusText = "Установка завершена!"
                    self.completion?()
                }
            } catch {
                DispatchQueue.main.async {
                    self.isDownloading = false
                    self.error = "Ошибка распаковки: \(error.localizedDescription)"
                }
            }
        }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        if let error = error {
            self.isDownloading = false
            self.error = "Ошибка загрузки: \(error.localizedDescription)"
        }
    }
}

// MARK: - Launcher UI

struct AlertItem: Identifiable {
    let id = UUID()
    let message: String
}

struct LauncherRootView: View {
    var onPlay: () -> Void
    @State private var tab: Int = 0
    @State private var hasFiles: Bool = GameFolder.hasGameFiles
    @State private var sensitivity: Double = GameFolder.readSensitivity()
    @State private var importingZip: Bool = false
    @State private var isUnzipping: Bool = false
    @State private var alertMessage: String? = nil
    @State private var linkURL: String = ""
    @StateObject private var downloader = ResourceDownloader()

    var body: some View {
        ZStack {
            LinearGradient(
                colors: [Color(red: 0.08, green: 0.10, blue: 0.14), Color(red: 0.12, green: 0.16, blue: 0.22)],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
            .ignoresSafeArea()

            VStack(spacing: 0) {
                headerView
                    .padding(.horizontal, 24)
                    .padding(.top, 16)
                    .padding(.bottom, 12)

                Picker("", selection: $tab) {
                    Text("Играть").tag(0)
                    Text("Настройки").tag(1)
                }
                .pickerStyle(.segmented)
                .padding(.horizontal, 24)
                .padding(.bottom, 16)

                if tab == 0 {
                    playTab
                } else {
                    settingsTab
                }
                Spacer(minLength: 0)
            }
        }
        .fileImporter(
            isPresented: $importingZip,
            allowedContentTypes: [.zip],
            allowsMultipleSelection: false
        ) { result in
            handleZipImport(result)
        }
        .alert(item: Binding<AlertItem?>(
            get: { alertMessage.map { AlertItem(message: $0) } },
            set: { alertMessage = $0?.message }
        )) { item in
            Alert(title: Text("Сообщение"), message: Text(item.message), dismissButton: .default(Text("OK")))
        }
    }

    private var headerView: some View {
        HStack(spacing: 12) {
            Image(systemName: "cube.fill")
                .font(.system(size: 26))
                .foregroundColor(.green)
            VStack(alignment: .leading, spacing: 2) {
                Text("Horizon")
                    .font(.system(size: 20, weight: .bold, design: .rounded))
                    .foregroundColor(.white)
                Text("Minecraft Java • Версия 0.7.0")
                    .font(.system(size: 11, weight: .medium))
                    .foregroundColor(.white.opacity(0.6))
            }
            Spacer()
        }
    }

    private var playTab: some View {
        VStack(spacing: 20) {
            Spacer()

            VStack(spacing: 8) {
                Text("Minecraft Java Edition")
                    .font(.system(size: 28, weight: .bold, design: .rounded))
                    .foregroundColor(.white)
                Text("Horizon Native Client (Rust)")
                    .font(.system(size: 14, weight: .medium, design: .monospaced))
                    .foregroundColor(.white.opacity(0.6))
            }

            HStack(spacing: 12) {
                Image(systemName: hasFiles ? "checkmark.circle.fill" : "exclamationmark.triangle.fill")
                    .font(.system(size: 24))
                    .foregroundColor(hasFiles ? .green : .orange)
                VStack(alignment: .leading, spacing: 2) {
                    Text(hasFiles ? "Готово к запуску" : "Пакеты игры не установлены")
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundColor(.white)
                    Text(hasFiles ? "Все необходимые ресурсы на месте" : "Перейдите в Настройки для выбора или загрузки zip")
                        .font(.system(size: 12))
                        .foregroundColor(.white.opacity(0.7))
                }
                Spacer()
            }
            .padding(16)
            .background(Color.white.opacity(0.08))
            .cornerRadius(14)
            .padding(.horizontal, 24)

            Button(action: {
                if hasFiles {
                    onPlay()
                } else {
                    alertMessage = "Сначала загрузите пакеты игры (horizon-resources.zip) во вкладке «Настройки»."
                }
            }) {
                HStack(spacing: 10) {
                    Image(systemName: "play.fill")
                        .font(.system(size: 20))
                    Text("ИГРАТЬ")
                        .font(.system(size: 20, weight: .bold, design: .rounded))
                }
                .frame(maxWidth: .infinity)
                .frame(height: 56)
                .background(hasFiles ? Color(red: 0.16, green: 0.65, blue: 0.27) : Color.gray.opacity(0.4))
                .foregroundColor(.white)
                .cornerRadius(16)
                .shadow(color: hasFiles ? Color.green.opacity(0.3) : Color.clear, radius: 10, y: 4)
            }
            .padding(.horizontal, 24)

            Spacer()
        }
    }

    private var settingsTab: some View {
        ScrollView {
            VStack(spacing: 16) {
                // Card 1: Sensitivity (Сенса)
                VStack(alignment: .leading, spacing: 12) {
                    HStack {
                        Image(systemName: "slider.horizontal.3")
                            .foregroundColor(.green)
                        Text("Чувствительность обзора (Сенса)")
                            .font(.system(size: 16, weight: .semibold))
                            .foregroundColor(.white)
                        Spacer()
                        Text("\(Int(sensitivity * 200))%")
                            .font(.system(size: 15, weight: .bold, design: .monospaced))
                            .foregroundColor(.green)
                    }

                    Text("Регулировка чувствительности поворота камеры при сенсорном обзоре.")
                        .font(.system(size: 12))
                        .foregroundColor(.white.opacity(0.65))

                    Slider(value: $sensitivity, in: 0.05...1.5, step: 0.01) { _ in
                        GameFolder.saveSensitivity(sensitivity)
                    }
                    .accentColor(.green)
                }
                .padding(16)
                .background(Color.white.opacity(0.08))
                .cornerRadius(14)

                // Card 2: Game Packages (Пакеты игры)
                VStack(alignment: .leading, spacing: 12) {
                    HStack {
                        Image(systemName: hasFiles ? "checkmark.seal.fill" : "archivebox.fill")
                            .foregroundColor(hasFiles ? .green : .orange)
                        Text("Пакеты игры (horizon-resources.zip)")
                            .font(.system(size: 16, weight: .semibold))
                            .foregroundColor(.white)
                        Spacer()
                        if hasFiles {
                            Text("Установлено")
                                .font(.system(size: 12, weight: .bold))
                                .foregroundColor(.green)
                                .padding(.horizontal, 8)
                                .padding(.vertical, 4)
                                .background(Color.green.opacity(0.15))
                                .cornerRadius(8)
                        }
                    }

                    if hasFiles {
                        Text("Пакеты игры успешно установлены в папку Documents/resources. Повторная установка не требуется.")
                            .font(.system(size: 13))
                            .foregroundColor(.white.opacity(0.5))

                        HStack(spacing: 12) {
                            Button(action: {}) {
                                HStack {
                                    Image(systemName: "checkmark")
                                    Text("Пакеты установлены")
                                }
                                .font(.system(size: 14, weight: .medium))
                                .frame(maxWidth: .infinity)
                                .frame(height: 44)
                                .background(Color.gray.opacity(0.2))
                                .foregroundColor(Color.white.opacity(0.4))
                                .cornerRadius(10)
                            }
                            .disabled(true)
                        }
                        .opacity(0.4)
                    } else {
                        Text("Для работы игры выберите подготовленный zip архив через Files или скачайте его по прямой ссылке.")
                            .font(.system(size: 13))
                            .foregroundColor(.white.opacity(0.75))

                        if isUnzipping {
                            HStack(spacing: 10) {
                                ProgressView()
                                    .progressViewStyle(CircularProgressViewStyle(tint: .white))
                                Text("Распаковка ресурсов...")
                                    .font(.system(size: 14))
                                    .foregroundColor(.white)
                            }
                            .padding(.vertical, 8)
                        } else if downloader.isDownloading {
                            VStack(alignment: .leading, spacing: 6) {
                                Text(downloader.statusText)
                                    .font(.system(size: 13))
                                    .foregroundColor(.white)
                                ProgressView(value: downloader.progress)
                                    .accentColor(.green)
                            }
                            .padding(.vertical, 8)
                        } else {
                            Button(action: {
                                importingZip = true
                            }) {
                                HStack {
                                    Image(systemName: "doc.badge.plus")
                                    Text("Выбрать ZIP архив из Файлов")
                                }
                                .font(.system(size: 14, weight: .semibold))
                                .frame(maxWidth: .infinity)
                                .frame(height: 44)
                                .background(Color.blue)
                                .foregroundColor(.white)
                                .cornerRadius(10)
                            }

                            Divider()
                                .background(Color.white.opacity(0.15))
                                .padding(.vertical, 4)

                            VStack(alignment: .leading, spacing: 6) {
                                Text("Скачать по ссылке:")
                                    .font(.system(size: 12, weight: .medium))
                                    .foregroundColor(.white.opacity(0.8))

                                HStack(spacing: 8) {
                                    TextField("https://example.com/horizon-resources.zip", text: $linkURL)
                                        .textFieldStyle(.plain)
                                        .padding(10)
                                        .background(Color.black.opacity(0.3))
                                        .foregroundColor(.white)
                                        .cornerRadius(8)
                                        .autocapitalization(.none)
                                        .disableAutocorrection(true)

                                    Button(action: {
                                        guard let url = URL(string: linkURL.trimmingCharacters(in: .whitespacesAndNewlines)),
                                              ["http", "https"].contains(url.scheme?.lowercased() ?? "") else {
                                            alertMessage = "Введите корректную ссылку, начинающуюся с https://"
                                            return
                                        }
                                        downloader.download(from: url) {
                                            self.hasFiles = GameFolder.hasGameFiles
                                        }
                                    }) {
                                        Text("Скачать")
                                            .font(.system(size: 13, weight: .semibold))
                                            .padding(.horizontal, 14)
                                            .frame(height: 38)
                                            .background(linkURL.isEmpty ? Color.gray.opacity(0.3) : Color.green)
                                            .foregroundColor(.white)
                                            .cornerRadius(8)
                                    }
                                    .disabled(linkURL.isEmpty)
                                }
                            }
                        }
                    }
                }
                .padding(16)
                .background(Color.white.opacity(0.08))
                .cornerRadius(14)
            }
            .padding(.horizontal, 24)
            .padding(.vertical, 10)
        }
    }

    private func handleZipImport(_ result: Result<[URL], Error>) {
        switch result {
        case .success(let urls):
            guard let url = urls.first else { return }
            isUnzipping = true
            DispatchQueue.global(qos: .userInitiated).async {
                let scoped = url.startAccessingSecurityScopedResource()
                defer { if scoped { url.stopAccessingSecurityScopedResource() } }
                do {
                    _ = try Unzip.resources(from: url, into: GameFolder.documents)
                    DispatchQueue.main.async {
                        self.isUnzipping = false
                        self.hasFiles = GameFolder.hasGameFiles
                        self.alertMessage = "Ресурсы успешно распакованы!"
                    }
                } catch {
                    DispatchQueue.main.async {
                        self.isUnzipping = false
                        self.alertMessage = "Ошибка распаковки: \(error.localizedDescription)"
                    }
                }
            }
        case .failure(let error):
            alertMessage = "Ошибка выбора файла: \(error.localizedDescription)"
        }
    }
}

// MARK: - Presenter and Entry Points

final class LauncherHostingController: UIHostingController<AnyView> {
    override var prefersStatusBarHidden: Bool { true }
    override var prefersHomeIndicatorAutoHidden: Bool { true }
    override var supportedInterfaceOrientations: UIInterfaceOrientationMask {
        UIDevice.current.userInterfaceIdiom == .pad ? .all : .landscape
    }
}

private var launcherPresented = false

func gameViewController() -> UIViewController? {
    let windows = UIApplication.shared.connectedScenes
        .filter { $0.session.role == .windowApplication }
        .compactMap { $0 as? UIWindowScene }
        .flatMap(\.windows)
    let window = windows.first(where: \.isKeyWindow) ?? windows.first
    var top = window?.rootViewController
    while let presented = top?.presentedViewController {
        top = presented
    }
    return top
}

private func presentLauncher(attempts: Int) {
    guard let root = gameViewController(), root.view.window != nil else {
        if attempts > 0 {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
                presentLauncher(attempts: attempts - 1)
            }
        }
        return
    }
    if launcherPresented || root.presentedViewController is LauncherHostingController {
        return
    }
    launcherPresented = true

    let host = LauncherHostingController(rootView: AnyView(EmptyView()))
    host.rootView = AnyView(LauncherRootView(onPlay: { [weak host] in
        guard let host else { return }
        host.dismiss(animated: true) {
            launcherPresented = false
        }
    }))
    host.modalPresentationStyle = .fullScreen
    host.modalTransitionStyle = .crossDissolve
    root.present(host, animated: false)
}

@_cdecl("horizon_launcher_present")
public func horizonLauncherPresent() -> Bool {
    DispatchQueue.main.async {
        presentLauncher(attempts: 30)
    }
    return true
}
