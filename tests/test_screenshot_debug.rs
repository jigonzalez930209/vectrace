use ashpd::desktop::screenshot::Screenshot;

#[test]
#[ignore]
fn test_screenshot_debug() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let req = Screenshot::request().interactive(true);
        let resp = req.send().await.unwrap().response().unwrap();
        println!("Response URI: {:?}", resp.uri());

        let uri = resp.uri();
        let raw_path = uri.as_str().trim_start_matches("file://");
        let path_str = urlencoding::decode(raw_path).unwrap();
        let bytes = std::fs::read(path_str.as_ref()).unwrap();
        let pixmap = tiny_skia::Pixmap::decode_png(&bytes).unwrap();
        println!("Decoded PNG Dimensions: {}x{}", pixmap.width(), pixmap.height());
    });
}
