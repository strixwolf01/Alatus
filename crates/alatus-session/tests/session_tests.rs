use alatus_session::AlatusTray;
use ksni::Tray;

#[test]
fn test_status_notifier_item_metadata() {
    let tray = AlatusTray {
        current_profile: "Performance".into(),
        current_limit: 80,
        current_refresh: 120,
    };

    assert_eq!(tray.id(), "alatus");
    assert_eq!(tray.title(), "Alatus Hardware Suite");
    assert_eq!(tray.icon_name(), "preferences-system-power");

    let tooltip = tray.tool_tip();
    assert!(tooltip.description.contains("Performance"));
    assert!(tooltip.description.contains("80%"));
    assert!(tooltip.description.contains("120Hz"));
}
