use ashpd::desktop::screenshot::Screenshot;

#[test]
#[ignore]
fn test_live_interactive_screenshot() {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let pixmap = rt.block_on(async {
        let response = Screenshot::request()
            .interactive(true)
            .send()
            .await
            .unwrap()
            .response()
            .unwrap();

        let uri = response.uri();
        let raw_path = uri.as_str().trim_start_matches("file://");
        let path_str = urlencoding::decode(raw_path).unwrap();
        let bytes = std::fs::read(path_str.as_ref()).unwrap();
        let _ = std::fs::remove_file(path_str.as_ref());
        tiny_skia::Pixmap::decode_png(&bytes).unwrap()
    });

    let save_path = "/tmp/test_interactive_desktop.png";
    pixmap.save_png(save_path).unwrap();
    println!("Saved interactive desktop image to: {}", save_path);

    let data = pixmap.data();
    let total_pixels = pixmap.width() as usize * pixmap.height() as usize;
    let mut sum_r: u64 = 0;
    let mut sum_g: u64 = 0;
    let mut sum_b: u64 = 0;

    for chunk in data.chunks_exact(4) {
        sum_r += chunk[0] as u64;
        sum_g += chunk[1] as u64;
        sum_b += chunk[2] as u64;
    }

    let avg_r = sum_r / total_pixels as u64;
    let avg_g = sum_g / total_pixels as u64;
    let avg_b = sum_b / total_pixels as u64;

    println!(
        "Interactive Pixel Stats: total={}, avg_r={}, avg_g={}, avg_b={}",
        total_pixels, avg_r, avg_g, avg_b
    );
}
