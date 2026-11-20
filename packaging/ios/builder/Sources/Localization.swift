import Foundation
import SwiftUI

/// Helpers for Russian / English bilingual interface in Horizon Mods.
enum L10n {
    @MainActor static func t(_ ru: String, _ en: String, store: Store) -> String {
        store.isRu ? ru : en
    }
}
