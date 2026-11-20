import SwiftUI
import UniformTypeIdentifiers

/// Horizon builds on this iPhone, to play inside Horizon Mods, and the fork's builds to get.
struct PlayView: View {
    @EnvironmentObject var store: Store
    @EnvironmentObject var library: Library
    @State private var deleting: LocalBuild?
    @State private var pickingZip = false
    @State private var importingZip = false
    @State private var zipError: String?

    private var available: [RemoteBuild] { library.remote.filter { !library.isLocal($0) } }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                PageHeader(
                    title: store.isRu ? "Играть" : "Play",
                    caption: store.isRu ? "Сборки Horizon на этом iPhone. Загрузите и играйте прямо здесь." : "Horizon builds on this iPhone. Get one, then play it right here."
                )
                gameFiles
                if let last = Launcher.lastError {
                    ErrorPlate(message: last) { Launcher.lastError = nil }
                }
                if let error = library.error {
                    ErrorPlate(message: error) { library.error = nil; Task { await library.refresh(gitHub: store.gitHubClient) } }
                }
                if !library.local.isEmpty {
                    Text(store.isRu ? "На этом iPhone" : "On this iPhone")
                        .font(.heading(20)).foregroundStyle(Theme.ink(1)).padding(.top, 4)
                    ForEach(library.local) { build in LocalBuildCard(build: build, delete: { deleting = build }) }
                }
                Text(store.isRu ? "Получить сборку" : "Get a build")
                    .font(.heading(20)).foregroundStyle(Theme.ink(1)).padding(.top, 4)
                if library.remote.isEmpty {
                    Plate {
                        if library.loading {
                            ProgressView().tint(Theme.ink(0.8)).frame(maxWidth: .infinity)
                        } else {
                            Text(store.isRu ? "Сборок не найдено на \(store.effectiveRepo)." : "No builds found on \(store.effectiveRepo).")
                                .font(.geist(14)).foregroundStyle(Theme.ink(0.7))
                        }
                    }
                }
                LazyVStack(spacing: 8) {
                    ForEach(available) { build in RemoteBuildRow(build: build) }
                }
            }
            .padding(.horizontal, 16)
            .padding(.bottom, 16)
        }
        .refreshable { await library.refresh(gitHub: store.gitHubClient) }
        .task { if library.remote.isEmpty { await library.refresh(gitHub: store.gitHubClient) } }
        .fileImporter(isPresented: $pickingZip, allowedContentTypes: [.zip]) { result in
            switch result {
            case .success(let url):
                guard url.startAccessingSecurityScopedResource() else {
                    zipError = store.isRu ? "Не удалось получить доступ к выбранному файлу" : "Could not access selected file"
                    return
                }
                defer { url.stopAccessingSecurityScopedResource() }
                importingZip = true
                Task {
                    do {
                        try library.importResources(from: url)
                        zipError = nil
                    } catch {
                        zipError = error.localizedDescription
                    }
                    importingZip = false
                }
            case .failure(let error):
                zipError = error.localizedDescription
            }
        }
        .confirmationDialog(
            store.isRu ? "Удалить \(deleting?.title ?? "эту сборку")?" : "Delete \(deleting?.title ?? "this build")?",
            isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } }),
            titleVisibility: .visible
        ) {
            Button(store.isRu ? "Удалить" : "Delete", role: .destructive) {
                if let deleting { library.delete(deleting) }
                deleting = nil
            }
        } message: {
            Text(store.isRu ? "Будет удалена только эта сборка. Ваши миры, моды и ресурсы останутся." : "Only the build goes. Your worlds, mods and game files stay.")
        }
    }

    private var gameFiles: some View {
        Plate {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Rectangle().fill(library.hasGameFiles ? Theme.live : Theme.warn).frame(width: 6, height: 6)
                Text(library.hasGameFiles
                     ? (store.isRu ? "Файлы игры на месте." : "Game files are in place.")
                     : (store.isRu ? "Требуются файлы игры" : "Game files needed"))
                    .font(.geist(15, .semibold)).foregroundStyle(Theme.ink(1))
            }
            Text(library.hasGameFiles
                 ? (store.isRu ? "Каждая сборка использует эти файлы, миры, моды и ресурс-паки." : "Every build shares them, with your worlds, mods and packs, in Files → On My iPhone → Horizon Mods.")
                 : (store.isRu ? "Выберите архив horizon-resources.zip или скопируйте папку resources в Файлы → На моем iPhone → Horizon Mods." : "Copy the resources folder (from horizon-resources.zip) into Files → On My iPhone → Horizon Mods, as you did for Horizon. Every build shares it, with your worlds, mods and packs."))
                .font(.geist(13)).foregroundStyle(Theme.ink(0.7))

            Button {
                pickingZip = true
            } label: {
                HStack(spacing: 8) {
                    if importingZip {
                        ProgressView().tint(Color.white).scaleEffect(0.8)
                        Text(store.isRu ? "Распаковка ресурсов..." : "Importing resources...")
                    } else {
                        HIcon(name: "package-01", size: 18)
                        Text(library.hasGameFiles
                             ? (store.isRu ? "Обновить horizon-resources.zip" : "Update horizon-resources.zip")
                             : (store.isRu ? "Выбрать horizon-resources.zip" : "Select horizon-resources.zip"))
                    }
                }
                .frame(maxWidth: .infinity)
            }
            .buttonStyle(AccentButton(height: 42))
            .disabled(importingZip)

            if let zipError {
                Text(zipError).font(.geist(12)).foregroundStyle(Theme.warn)
            }
        }
    }
}

struct LocalBuildCard: View {
    @EnvironmentObject var store: Store
    @EnvironmentObject var library: Library
    @State private var starting = false
    @State private var problem: String?
    let build: LocalBuild
    let delete: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 12) {
                Image(uiImage: UIImage(named: "mark") ?? UIImage())
                    .resizable().interpolation(.none).padding(9)
                    .frame(width: 50, height: 50)
                    .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Theme.fill(0.08)))
                VStack(alignment: .leading, spacing: 2) {
                    Text(build.title).font(.heading(21)).foregroundStyle(Theme.ink(1))
                    Text(build.mods.isEmpty ? (store.isRu ? "Без нативных модов" : "No native mods") : store.names(build.mods))
                        .font(.geist(13)).foregroundStyle(Theme.ink(0.65)).lineLimit(2)
                    Text("\(build.date.formatted(date: .abbreviated, time: .omitted)) · \(ByteCountFormatter.string(fromByteCount: build.bytes, countStyle: .file)) · Builds/\(build.folder)")
                        .font(.mono(11)).foregroundStyle(Theme.ink(0.45))
                }
                Spacer(minLength: 0)
                Menu {
                    Button(role: .destructive, action: delete) {
                        Label(store.isRu ? "Удалить" : "Delete", systemImage: "trash")
                    }
                } label: {
                    HIcon(name: "more-horizontal", size: 20).foregroundStyle(Theme.ink(0.7)).frame(width: 36, height: 36)
                }
            }
            Button {
                Task {
                    starting = true
                    do {
                        try await Launcher.play(build, library: library)
                    } catch {
                        problem = error.localizedDescription
                    }
                    starting = false
                }
            } label: {
                HStack(spacing: 8) {
                    if starting { ProgressView().tint(Color.white) } else { HIcon(name: "play", size: 18) }
                    Text(starting
                         ? (build.signedWith == nil ? (store.isRu ? "Подпись…" : "Signing…") : (store.isRu ? "Запуск…" : "Starting…"))
                         : (store.isRu ? "Играть" : "Play"))
                }
                .frame(maxWidth: .infinity)
            }
            .buttonStyle(AccentButton(height: 46))
            .disabled(starting)
            if let problem {
                Text(problem).font(.geist(12)).foregroundStyle(Theme.warn)
            } else {
                Text(store.isRu
                     ? "Запуск внутри Horizon Mods. Чтобы вернуться сюда, закройте игру (свайпом) и снова откройте Horizon Mods."
                     : "Plays inside Horizon Mods. To come back here, close the game (swipe it away) and open Horizon Mods again.")
                    .font(.geist(12)).foregroundStyle(Theme.ink(0.55))
            }
        }
        .padding(16)
        .glass()
    }
}

struct RemoteBuildRow: View {
    @EnvironmentObject var store: Store
    @EnvironmentObject var library: Library
    let build: RemoteBuild

    var body: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text(build.title).font(.geist(16, .semibold)).foregroundStyle(Theme.ink(1))
                if build.native {
                    Text(build.mods.isEmpty ? (store.isRu ? "Нативные моды" : "Native mods") : store.names(build.mods))
                        .font(.geist(13)).foregroundStyle(Theme.accentSoft).lineLimit(2)
                }
                Text("\(build.date.formatted(date: .abbreviated, time: .shortened)) · \(ByteCountFormatter.string(fromByteCount: Int64(build.size), countStyle: .file))")
                    .font(.mono(11)).foregroundStyle(Theme.ink(0.5))
            }
            Spacer(minLength: 0)
            if let fraction = library.progress[build.tag] {
                VStack(alignment: .trailing, spacing: 4) {
                    Text(fraction > 0.9
                         ? (store.isRu ? "Распаковка" : "Unpacking")
                         : "\(Int(fraction * 100))%").font(.mono(12)).foregroundStyle(Theme.ink(0.75))
                    ProgressBar(value: fraction).frame(width: 80)
                }
            } else {
                Menu {
                    ForEach(Sideload.links(build.url)) { link in
                        Link(destination: link.url) {
                            Label(
                                (store.isRu ? "Установить как отдельное приложение: " : "Install as its own app: ") + link.label.replacingOccurrences(of: "Open in ", with: ""),
                                systemImage: "arrow.down.app"
                            )
                        }
                    }
                } label: {
                    HIcon(name: "more-horizontal", size: 20).foregroundStyle(Theme.ink(0.6)).frame(width: 32, height: 38)
                }
                Button {
                    Task { await library.get(build) }
                } label: {
                    HStack(spacing: 6) {
                        HIcon(name: "download-04", size: 16)
                        Text(store.isRu ? "Скачать" : "Get")
                    }
                }
                .buttonStyle(AccentButton(height: 38))
            }
        }
        .padding(12)
        .glass(Theme.Radius.popover)
    }
}
