use alatus_session::StatusNotifierItemService;

#[test]
fn test_status_notifier_item_metadata() {
    let tray = StatusNotifierItemService::new();
    tray.set_profile("Performance");

    // Tray properties can be verified
    assert_eq!(tray.category(), "Hardware");
    assert_eq!(tray.id(), "alatus");
    assert_eq!(tray.title(), "Alatus Hardware Suite");
    assert_eq!(tray.status(), "Active");
    assert_eq!(tray.icon_name(), "preferences-system-power");
}
