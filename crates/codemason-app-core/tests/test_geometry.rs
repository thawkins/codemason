//! Tests for `codemason_app_core::geometry`.

use codemason_app_core::geometry::Area;

#[test]
fn test_area_new_clamps_to_u16_edges() {
    let a = Area::new(u16::MAX - 2, 5, 10, 3);
    assert_eq!(a.width, 2);
    assert_eq!(a.right(), u16::MAX);
    assert_eq!(a.bottom(), 8);
}

#[test]
fn test_area_contains_is_half_open() {
    let a = Area::new(2, 3, 4, 2);
    assert!(a.contains((2, 3)));
    assert!(a.contains((5, 4)));
    assert!(!a.contains((6, 4)));
    assert!(!a.contains((5, 5)));
    assert!(!a.contains((1, 3)));
    assert!(!Area::default().contains((0, 0)));
}

#[test]
fn test_area_accessors() {
    let a = Area::new(1, 2, 3, 4);
    assert_eq!((a.left(), a.top(), a.right(), a.bottom()), (1, 2, 4, 6));
    assert_eq!(a.area(), 12);
    assert!(!a.is_empty());
    assert!(Area::new(1, 2, 0, 4).is_empty());
}
