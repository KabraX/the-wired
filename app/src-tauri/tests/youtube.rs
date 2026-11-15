//! Ficha, búsqueda y argumentos de descarga de YouTube, sin red (JSON guardado).

use std::ffi::OsString;
use std::path::Path;

use the_wired_lib::jkanime::Site;
use the_wired_lib::youtube::{self, AudioCodec, YtFormat};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
    )
    .unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

#[test]
fn youtube_urls() {
    assert!(youtube::is_youtube_url(
        "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
    ));
    assert!(youtube::is_youtube_url("https://youtu.be/dQw4w9WgXcQ?t=10"));
    assert!(youtube::is_youtube_url(
        "https://www.youtube.com/playlist?list=PL123"
    ));
    assert!(!youtube::is_youtube_url("https://jkanime.net/elfen-lied/"));
    assert!(!youtube::is_youtube_url("elfen lied"));
    assert!(!youtube::is_youtube_url(""));
}

#[test]
fn video_urls_only() {
    assert!(youtube::is_video_url(
        "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
    ));
    assert!(youtube::is_video_url(
        "https://www.youtube.com/shorts/dQw4w9WgXcQ"
    ));
    assert!(youtube::is_video_url(
        "https://www.youtube.com/live/dQw4w9WgXcQ"
    ));
    assert!(youtube::is_video_url("https://youtu.be/dQw4w9WgXcQ"));
    assert!(!youtube::is_video_url(
        "https://www.youtube.com/playlist?list=PL123"
    ));
    assert!(!youtube::is_video_url("https://www.youtube.com/@canal"));
    // Ficha: solo vídeos y playlists (un canal tiene miles de entradas)
    assert!(youtube::is_supported_url(
        "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
    ));
    assert!(youtube::is_supported_url(
        "https://www.youtube.com/playlist?list=PL123"
    ));
    assert!(!youtube::is_supported_url(
        "https://www.youtube.com/@un-canal"
    ));
}

#[test]
fn search_json_maps_to_results() {
    let r = youtube::parse_search_json(&fixture("yt-search.json")).unwrap();
    assert_eq!(r.len(), 2, "el canal no es un resultado descargable");
    assert_eq!(r[0].source, Site::Youtube);
    assert_eq!(r[0].title, "Midnight Drive");
    assert_eq!(r[0].url, "https://www.youtube.com/watch?v=abc123");
    assert_eq!(
        r[0].image.as_deref(),
        Some("https://i.ytimg.com/vi/abc123/hqdefault.jpg")
    );
    assert_eq!(r[0].kind.as_deref(), Some("4:05"));
    assert_eq!(r[0].status.as_deref(), Some("Neon Channel"));

    // El id construye la URL y la miniatura cuando faltan
    assert_eq!(r[1].url, "https://www.youtube.com/watch?v=def456");
    assert_eq!(
        r[1].image.as_deref(),
        Some("https://cdn.example/rainy.jpg"),
        "thumbnail tiene prioridad"
    );
    assert_eq!(r[1].kind.as_deref(), Some("1:00:25"));
    assert_eq!(r[1].status.as_deref(), Some("Lo-Fi Boy"));
}

#[test]
fn single_video_info() {
    let info =
        youtube::parse_info_json(&fixture("yt-video.json"), "https://youtu.be/vid1").unwrap();
    assert!(!info.is_playlist);
    assert_eq!(info.title, "A Single Video");
    assert_eq!(info.url, "https://www.youtube.com/watch?v=vid1");
    assert_eq!(info.channel.as_deref(), Some("Some Channel"));
    assert_eq!(info.duration, Some(192));
    assert_eq!(info.entries.len(), 1);
    assert_eq!(info.entries[0].index, 1);
    assert_eq!(info.entries[0].id, "vid1");
}

#[test]
fn playlist_info_skips_private_videos() {
    let info = youtube::parse_info_json(
        &fixture("yt-playlist.json"),
        "https://www.youtube.com/playlist?list=PL1",
    )
    .unwrap();
    assert!(info.is_playlist);
    assert_eq!(info.title, "My Playlist");
    assert_eq!(info.channel.as_deref(), Some("List Owner"));
    let titles: Vec<_> = info.entries.iter().map(|e| e.title.as_str()).collect();
    assert_eq!(titles, ["First", "Third"]);
    // El índice de la playlist se conserva (2 era privado) y la URL sale del id
    assert_eq!(info.entries[0].index, 1);
    assert_eq!(info.entries[1].index, 3);
    assert_eq!(info.entries[1].url, "https://www.youtube.com/watch?v=a3");
}

#[test]
fn invalid_json_is_a_clear_error() {
    assert!(youtube::parse_search_json("no es json").is_err());
    assert!(youtube::parse_info_json("no es json", "https://youtu.be/x").is_err());
    let no_title = youtube::parse_info_json(r#"{"_type":"video","id":"x"}"#, "https://youtu.be/x");
    assert!(no_title.is_err(), "sin título no se puede mostrar");
}

#[test]
fn download_args_for_video_and_audio() {
    let template = Path::new("/lib/YouTube/Un vídeo.%(ext)s");
    let progress = "download:TW|%(progress.downloaded_bytes)s";
    let args = youtube::download_args(
        "https://youtu.be/abc",
        template,
        &YtFormat::Video {
            max_height: Some(1080),
        },
        progress,
    );
    let text: Vec<String> = args
        .iter()
        .map(|a: &OsString| a.to_string_lossy().into())
        .collect();
    let joined = text.join(" ");
    assert!(joined.contains("-f bv*[ext=mp4][height<=1080]+ba[ext=m4a]"));
    assert!(joined.contains("--merge-output-format mp4"));
    assert!(joined.contains("--remux-video mp4"));
    assert!(joined.contains("--no-playlist"));
    assert_eq!(
        text.last().map(String::as_str),
        Some("https://youtu.be/abc")
    );
    assert!(text.contains(&"/lib/YouTube/Un vídeo.%(ext)s".to_string()));

    let args = youtube::download_args(
        "https://youtu.be/abc",
        template,
        &YtFormat::Audio {
            codec: AudioCodec::Mp3,
        },
        progress,
    );
    let text: Vec<String> = args.iter().map(|a| a.to_string_lossy().into()).collect();
    let joined = text.join(" ");
    assert!(joined.contains("-x --audio-format mp3 --audio-quality 0"));
    assert_eq!(
        YtFormat::Audio {
            codec: AudioCodec::Mp3
        }
        .extension(),
        "mp3"
    );
    assert_eq!(
        YtFormat::Audio {
            codec: AudioCodec::M4a
        }
        .extension(),
        "m4a"
    );
    assert_eq!(YtFormat::Video { max_height: None }.extension(), "mp4");
}

#[test]
fn permanent_failures_are_not_retried() {
    assert!(youtube::is_permanent_error(
        "ERROR: [youtube] x: Private video. Sign in if you've been granted access"
    ));
    assert!(youtube::is_permanent_error(
        "ERROR: Video unavailable. This video has been removed by the uploader"
    ));
    assert!(!youtube::is_permanent_error(
        "HTTP Error 503: Service Unavailable"
    ));
    assert!(!youtube::is_permanent_error("read timed out"));
}
