import Foundation

/// Native text in the phone's language. The Vietnamese default written at the
/// call site is the base (the development region is `vi`); en.lproj and
/// zh-Hans.lproj/Localizable.strings translate it under the same key.
/// `web/scripts/native-strings.test.ts` checks that every key has both.
func localized(_ key: String, _ vietnamese: String) -> String {
    NSLocalizedString(key, value: vietnamese, comment: "")
}

/// `localized` with `%@` placeholders filled in order.
func localized(_ key: String, _ vietnamese: String, _ args: String...) -> String {
    String(format: localized(key, vietnamese), arguments: args)
}
