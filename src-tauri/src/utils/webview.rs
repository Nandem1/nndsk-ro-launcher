//! Ajustes del WebView GTK/WebKit en Linux (Wayland/Hyprland).

/// Evita pantalla negra / fallos GBM del WebView en Wayland.
///
/// Debe ejecutarse antes de que Tauri inicialice GTK/WebKit (AppImage incluido).
#[cfg(target_os = "linux")]
pub fn configure_linux_webview_env() {
    if std::env::var_os("GDK_BACKEND").is_none() {
        // SAFETY: single-threaded al arranque, antes de cargar GTK/WebKit.
        unsafe { std::env::set_var("GDK_BACKEND", "x11") };
    }
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }
}

#[cfg(not(target_os = "linux"))]
pub fn configure_linux_webview_env() {}
