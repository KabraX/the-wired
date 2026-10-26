//! Instalación de herramientas en un equipo "limpio".
//!   cargo test --test tools -- --ignored --nocapture   (descarga yt-dlp real, ~40 MB)

// Usa `tauri::test::mock_app`: en Windows los binarios de test no llevan el manifiesto
// de la app y Windows no los arranca (STATUS_ENTRYPOINT_NOT_FOUND). La lógica es la
// misma en ambos sistemas, así que estos tests se ejecutan solo fuera de Windows.
#![cfg(not(windows))]

use the_wired_lib::tools::{self, Source};

#[tokio::test]
#[ignore = "descarga yt-dlp de GitHub"]
async fn installs_ytdlp_when_missing() {
    let base = std::env::temp_dir().join(format!("the-wired-tools-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(base.join("empty-path")).unwrap();
    // Equipo sin yt-dlp: PATH apuntando a una carpeta vacía
    // SAFETY: este binario de test solo tiene esta prueba; nadie más lee el entorno a la vez
    unsafe { std::env::set_var("PATH", base.join("empty-path")) };
    tools::init(&base);

    let before = tools::status().await;
    println!("antes: {:?}", before.ytdlp.source);
    assert_eq!(before.ytdlp.source, Source::Missing);

    let app = tauri::test::mock_app();
    let after = tools::install_missing(app.handle())
        .await
        .expect("instalación");
    println!(
        "después: {:?} versión {:?}",
        after.ytdlp.source, after.ytdlp.version
    );
    assert_eq!(after.ytdlp.source, Source::App);
    assert!(
        after.ytdlp.version.is_some(),
        "el yt-dlp descargado debe ejecutarse"
    );

    // Con su propia copia, la actualización funciona
    println!("update: {:?}", tools::update_ytdlp().await);
    let _ = std::fs::remove_dir_all(&base);
}
