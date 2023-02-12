use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, gio, glib, graphene, gsk};
use std::cell::{Cell, RefCell};

const GRADIENT_SHADER: &[u8] = r#"
// That shader was taken from Telegram for android source
// https://github.com/DrKLO/Telegram/commit/2112affb2e4941334f8fbc3944385806b3c4e3d6#diff-dfdd1e8c4691747fd30199b7a2f5041a126b23e1450b29afe441eb0ebed01c68

precision highp float;

uniform vec3 color1;
uniform vec3 color2;
uniform vec3 color3;
uniform vec3 color4;
uniform vec2 p1;
uniform vec2 p2;
uniform vec2 p3;
uniform vec2 p4;

void mainImage(out vec4 fragColor,
               in vec2 fragCoord,
               in vec2 resolution,
               in vec2 uv) {
    uv.y = 1.0 - uv.y;

    float dp1 = distance(uv, p1);
    float dp2 = distance(uv, p2);
    float dp3 = distance(uv, p3);
    float dp4 = distance(uv, p4);
    float minD = min(dp1, min(dp2, min(dp3, dp4)));
    float p = 5.0;
    dp1 = pow(1.0 - (dp1 - minD), p);
    dp2 = pow(1.0 - (dp2 - minD), p);
    dp3 = pow(1.0 - (dp3 - minD), p);
    dp4 = pow(1.0 - (dp4 - minD), p);
    float sumDp = dp1 + dp2 + dp3 + dp4;

    vec3 color = (color1 * dp1 + color2 * dp2 + color3 * dp3 + color4 * dp4) / sumDp;
    fragColor = vec4(color, 1.0);
}
"#
.as_bytes();

const PATTERN_SHADER: &[u8] = r#"
precision highp float;
precision highp sampler2D;

uniform bool dark;
uniform sampler2D u_texture1;
uniform sampler2D u_texture2;

bool approxEq(float a, float b) {
    return abs(a - b) < 0.05;
}

void mainImage(out vec4 fragColor,
    in vec2 fragCoord,
    in vec2 resolution,
    in vec2 uv) {

    vec4 messages = GskTexture(u_texture1, uv);
    vec4 pattern = GskTexture(u_texture2, uv);

    float message_alpha = dark ? 0.9 : 0.8;
    float event_alpha = dark ? 0.8 : 0.2;

    // We don't need to draw pattern under semi-transparent messages
    // But we need to draw it under antialized corners
    if (
        approxEq(messages.a, message_alpha) ||
        approxEq(messages.a, event_alpha) ||
        messages.a == 1.0
    ) {
        fragColor = messages;
    } else {
        vec4 pattern_color;

        if (dark) {
            float alpha = 1.0 - pattern.a * 0.3;
            pattern_color = vec4(vec3(30.0 / 255.0), 1.0) * alpha;
        } else {
            float alpha = pattern.a * 0.1;
            pattern_color = vec4(vec3(0.0), 1.0) * alpha;
        }

        // blend colors with premultiplied alpha
        fragColor = messages + pattern_color * (1.0 - messages.a);
    }
}
"#
.as_bytes();

mod imp {
    use super::*;
    use glib::clone;
    use glib::once_cell::unsync::OnceCell;

    #[derive(Default)]
    pub struct Background {
        pub(super) gradient_texture: RefCell<Option<gdk::Texture>>,
        pub(super) last_size: Cell<(f32, f32)>,

        pub(super) shaders: RefCell<Option<Option<[gsk::GLShader; 2]>>>,
        pub(super) pattern: OnceCell<gdk::Texture>,

        pub(super) animation: OnceCell<adw::Animation>,
        pub(super) progress: Cell<f32>,
        pub(super) phase: Cell<u32>,

        pub(super) dark: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Background {
        const NAME: &'static str = "ContentBackground";
        type Type = super::Background;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for Background {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            let pattern =
                gdk::Texture::from_resource("/com/github/melix99/telegrand/images/pattern.svg");

            self.pattern.set(pattern).unwrap();

            let style_manager = adw::StyleManager::default();
            self.dark.set(style_manager.is_dark());

            style_manager.connect_dark_notify(clone!(@weak obj => move |style_manager| {
                let imp = obj.imp();
                imp.dark.set(style_manager.is_dark());
                imp.gradient_texture.take();
            }));

            let target = adw::CallbackAnimationTarget::new(clone!(@weak obj => move |progress| {
                let imp = obj.imp();
                imp.gradient_texture.take();
                let progress = progress as f32;
                if progress >= 1.0 {
                    imp.progress.set(0.0);
                    imp.phase.set((imp.phase.get() + 1) % 8);
                } else {
                    imp.progress.set(progress)
                }
                obj.queue_draw();
            }));

            let animation = adw::TimedAnimation::new(obj.as_ref(), 0.0, 1.0, 200, target);
            animation.set_easing(adw::Easing::EaseInOutQuad);
            self.animation.set(animation.upcast()).unwrap();
        }
    }

    impl WidgetImpl for Background {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            widget.ensure_shader();

            if let Some(None) = &*self.shaders.borrow() {
                // fallback code
                if let Some(child) = widget.child() {
                    widget.snapshot_child(&child, snapshot);
                }
                return;
            };

            let width = widget.width() as f32;
            let height = widget.height() as f32;

            if width == 0.0 || height == 0.0 {
                return;
            }

            let bounds = graphene::Rect::new(0.0, 0.0, width, height);

            let size_changed = self.last_size.replace((width, height)) != (width, height);

            self.snapshot_gradient(snapshot, &bounds, size_changed);

            self.snapshot_pattern(snapshot, &bounds);
        }
    }

    impl BinImpl for Background {}

    impl Background {
        fn snapshot_gradient(
            &self,
            snapshot: &gtk::Snapshot,
            bounds: &graphene::Rect,
            size_changed: bool,
        ) {
            if self.progress.get() == 0.0 {
                let texture = {
                    let cache = self.gradient_texture.borrow();

                    match &*cache {
                        Some(texture) if !size_changed => texture.clone(),
                        _ => {
                            drop(cache);

                            let renderer = self.obj().native().unwrap().renderer();
                            let texture = renderer
                                .render_texture(self.gradient_shader_node(bounds), Some(bounds));

                            self.gradient_texture.replace(Some(texture.clone()));

                            texture
                        }
                    }
                };

                snapshot.append_texture(&texture, bounds);
            } else {
                snapshot.append_node(self.gradient_shader_node(bounds));
            }
        }

        fn snapshot_pattern(&self, snapshot: &gtk::Snapshot, bounds: &graphene::Rect) {
            let widget = self.obj();
            let Some(Some([_, pattern_shader])) = &*self.shaders.borrow() else {unreachable!()};

            let args_builder = gsk::ShaderArgsBuilder::new(pattern_shader, None);

            args_builder.set_bool(0, self.dark.get());

            snapshot.push_gl_shader(pattern_shader, bounds, args_builder.to_args());

            if let Some(child) = widget.child() {
                widget.snapshot_child(&child, snapshot);
            }
            snapshot.gl_shader_pop_texture();

            let pattern = self.pattern.get().unwrap();

            let pattern_bounds = graphene::Rect::new(
                0.0,
                0.0,
                pattern.width() as f32 * 0.3,
                pattern.height() as f32 * 0.3,
            );

            snapshot.push_repeat(bounds, Some(&pattern_bounds));
            snapshot.append_texture(pattern, &pattern_bounds);
            snapshot.pop();
            snapshot.gl_shader_pop_texture();

            snapshot.pop();
        }

        fn gradient_shader_node(&self, bounds: &graphene::Rect) -> gsk::GLShaderNode {
            let Some(Some([gradient_shader, _])) = &*self.shaders.borrow() else {
                unreachable!()
            };

            let args_builder = gsk::ShaderArgsBuilder::new(gradient_shader, None);

            let dark = self.dark.get();
            let progress = self.progress.get();
            let phase = self.phase.get() as usize;

            let [c1, c2, c3, c4] = Self::colors(dark);
            args_builder.set_vec3(0, &c1);
            args_builder.set_vec3(1, &c2);
            args_builder.set_vec3(2, &c3);
            args_builder.set_vec3(3, &c4);

            let [p1, p2, p3, p4] = Self::calculate_positions(progress, phase);
            args_builder.set_vec2(4, &p1);
            args_builder.set_vec2(5, &p2);
            args_builder.set_vec2(6, &p3);
            args_builder.set_vec2(7, &p4);

            gsk::GLShaderNode::new(gradient_shader, bounds, &args_builder.to_args(), &[])
        }

        fn colors(dark: bool) -> [graphene::Vec3; 4] {
            use graphene::Vec3;
            if dark {
                [
                    Vec3::new(0.99607843, 0.76862746, 0.5882353),
                    Vec3::new(0.8666667, 0.42352942, 0.7254902),
                    Vec3::new(0.5882353, 0.18431373, 0.7490196),
                    Vec3::new(0.30980393, 0.35686275, 0.8352941),
                ]
            } else {
                [
                    Vec3::new(0.85882354, 0.8666667, 0.73333335),
                    Vec3::new(0.41960785, 0.64705884, 0.5294118),
                    Vec3::new(0.8352941, 0.84705883, 0.5529412),
                    Vec3::new(0.53333336, 0.72156864, 0.5176471),
                ]
            }
        }

        fn calculate_positions(progress: f32, phase: usize) -> [graphene::Vec2; 4] {
            static POSITIONS: [(f32, f32); 8] = [
                (0.80, 0.10),
                (0.60, 0.20),
                (0.35, 0.25),
                (0.25, 0.60),
                (0.20, 0.90),
                (0.40, 0.80),
                (0.65, 0.75),
                (0.75, 0.40),
            ];

            let mut points = [graphene::Vec2::new(0.0, 0.0); 4];

            for i in 0..4 {
                let start = POSITIONS[(i * 2 + phase) % 8];
                let end = POSITIONS[(i * 2 + phase + 1) % 8];

                fn interpolate(start: f32, end: f32, value: f32) -> f32 {
                    start + ((end - start) * value)
                }

                let x = interpolate(start.0, end.0, progress);
                let y = interpolate(start.1, end.1, progress);

                points[i] = graphene::Vec2::new(x, y);
            }

            points
        }
    }
}

glib::wrapper! {
    pub struct Background(ObjectSubclass<imp::Background>)
        @extends gtk::Widget, adw::Bin;
}

impl Background {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn animate(&self) {
        let animation = self.imp().animation.get().unwrap();

        let val = animation.value();
        if val == 0.0 || val == 1.0 {
            animation.play()
        }
    }

    fn ensure_shader(&self) {
        let imp = self.imp();
        if imp.shaders.borrow().is_none() {
            let renderer = self.native().unwrap().renderer();

            let sources = [GRADIENT_SHADER, PATTERN_SHADER];

            let shaders: Vec<_> = sources
                .iter()
                .flat_map(|source| {
                    let shader = gsk::GLShader::from_bytes(&source.into());
                    if let Err(e) = shader.compile(&renderer) {
                        if !e.matches(gio::IOErrorEnum::NotSupported) {
                            log::error!("can't compile shader for gradient background {e}");
                        }
                        return None;
                    }
                    Some(shader)
                })
                .collect();

            let shaders = shaders.try_into().ok();

            if shaders.is_none() {
                if let Some(c) = self.child() {
                    c.add_css_class("fallback")
                }
            }

            imp.shaders.replace(Some(shaders));
        }
    }
}

impl Default for Background {
    fn default() -> Self {
        Self::new()
    }
}
