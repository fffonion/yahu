#[test]
fn png_heic_generation_is_independent_of_source_size() {
    let small_png = Path::new("small.png");
    let empty_png = Path::new("empty.PNG");
    let jpeg = Path::new("photo.jpg");

    assert!(source_can_generate_heic(small_png));
    assert!(source_can_generate_heic(empty_png));
    assert_eq!(heic_status_for_source(small_png, false), "missing");
    assert_eq!(heic_status_for_source(empty_png, false), "missing");
    assert_eq!(heic_status_for_source(jpeg, false), "not_applicable");
    assert_eq!(heic_status_for_source(small_png, true), "available");
}
