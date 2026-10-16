import Capacitor
import UIKit
import WebKit

/// Capacitor bridge of the app: registers the plugins that live in this
/// target (Capacitor only discovers packaged plugins on its own) and turns
/// on the edge swipe back/forward through the web app's history (pages
/// have URLs; Capacitor leaves the gesture off by default).
class MainViewController: CAPBridgeViewController {
    override open func capacitorDidLoad() {
        webView?.allowsBackForwardNavigationGestures = true
        bridge?.registerPluginInstance(HoandatAuthPlugin())
        bridge?.registerPluginInstance(NativeFilesPlugin())
        bridge?.registerPluginInstance(RecorderPlugin())
        bridge?.registerPluginInstance(SecretsPlugin())
    }
}
