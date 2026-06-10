use std::cell::RefCell;
use std::collections::HashSet;

pub const BUILTIN_GAME_NATIVE_TYPES: &[&str] = &[
    "Vector2!", "Vector3!", "Vector4!",
    "Quat!", "Color!", "Rect!", "Ray!",
    "Transform!", "Matrix4!",
];

thread_local! {
    static USER_DEFINED: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameNativeLayout {
    Vec2F32,
    Vec3F32,
    Vec4F32,
    QuatF32,
    ColorF32,
    RectF32,
    RayF32,
    Transform,
    Matrix4F32,
}

pub fn is_known(name: &str) -> bool {
    if BUILTIN_GAME_NATIVE_TYPES.contains(&name) {
        return true;
    }
    USER_DEFINED.with(|s| s.borrow().contains(name))
}

pub fn register(name: &str) {
    USER_DEFINED.with(|s| {
        s.borrow_mut().insert(name.to_string());
    });
}

pub fn layout_for(name: &str) -> Option<GameNativeLayout> {
    match name {
        "Vector2!" => Some(GameNativeLayout::Vec2F32),
        "Vector3!" => Some(GameNativeLayout::Vec3F32),
        "Vector4!" => Some(GameNativeLayout::Vec4F32),
        "Quat!" => Some(GameNativeLayout::QuatF32),
        "Color!" => Some(GameNativeLayout::ColorF32),
        "Rect!" => Some(GameNativeLayout::RectF32),
        "Ray!" => Some(GameNativeLayout::RayF32),
        "Transform!" => Some(GameNativeLayout::Transform),
        "Matrix4!" => Some(GameNativeLayout::Matrix4F32),
        _ => None,
    }
}
