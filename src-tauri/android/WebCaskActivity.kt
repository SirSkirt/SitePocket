package __PACKAGE__

// Every Tauri WebviewWindow on Android is backed by an Activity.
// This is the dedicated, unprivileged WebCask game activity; the manager
// remains in MainActivity and can be returned to with Android Back.
class WebCaskActivity : TauriActivity()
