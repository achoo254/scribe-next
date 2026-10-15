import Capacitor
import UIKit

/// Capacitor bridge of the app: registers the plugins that live in this
/// target (Capacitor only discovers packaged plugins on its own).
class MainViewController: CAPBridgeViewController {
    override open func capacitorDidLoad() {
        bridge?.registerPluginInstance(GoogleAuthPlugin())
        bridge?.registerPluginInstance(NativeFilesPlugin())
        bridge?.registerPluginInstance(RecorderPlugin())
        bridge?.registerPluginInstance(SecretsPlugin())
    }
}
