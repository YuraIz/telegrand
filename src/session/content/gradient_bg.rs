use adw::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};
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

void mainImage(out vec4 fragColor,
    in vec2 fragCoord,
    in vec2 resolution,
    in vec2 uv) {

    vec4 messages = GskTexture(u_texture1, uv);
    vec4 pattern = GskTexture(u_texture2, uv);

    float message_alpha = dark ? 0.9 : 0.8;

    // We don't need to draw pattern under semi-transparent messages
    // But we need to draw it under antialized corners
    if ((abs(messages.a - message_alpha) > 0.004) && messages.a != 1.0) {
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
    } else {
        fragColor = messages;
    }
}
"#
.as_bytes();

mod imp {
    use super::*;
    use gtk::glib::once_cell::sync::Lazy;
    use gtk::glib::subclass::Signal;
    use gtk::glib::{self, clone};

    struct TextureCache<T: PartialEq> {
        texture: RefCell<Option<gdk::Texture>>,
        params: RefCell<T>,
    }

    impl<T: PartialEq> TextureCache<T> {
        fn try_get(&self, params: T) -> Option<gdk::Texture> {
            if params == *self.params.borrow() {
                self.texture.borrow().clone()
            } else {
                None
            }
        }

        fn save(&self, params: T, texture: Option<gdk::Texture>) {
            if params != *self.params.borrow() {
                self.texture.replace(texture);
                self.params.replace(params);
            }
        }
    }

    #[derive(PartialEq, Clone, Copy)]
    struct GradientCacheParams {
        size: (f32, f32),
        colors: [graphene::Vec3; 4],
        phase: u32,
    }

    pub struct GradientBackground {
        gradient_cache: TextureCache<GradientCacheParams>,

        pub(super) shaders: RefCell<Option<Option<[gsk::GLShader; 2]>>>,
        pub(super) pattern: RefCell<gdk::Texture>,

        pub(super) progress: Cell<f32>,
        pub(super) phase: Cell<u32>,

        pub(super) color1: Cell<graphene::Vec3>,
        pub(super) color2: Cell<graphene::Vec3>,
        pub(super) color3: Cell<graphene::Vec3>,
        pub(super) color4: Cell<graphene::Vec3>,
        pub(super) dark: Cell<bool>,
    }

    impl Default for GradientBackground {
        fn default() -> Self {
            let pattern =
                gdk::Texture::from_resource("/com/github/melix99/telegrand/images/pattern.svg");

            let color1 = graphene::Vec3::new(0.1, 1.0, 0.5);
            let color2 = graphene::Vec3::new(1.0, 1.0, 0.5);
            let color3 = graphene::Vec3::new(0.1, 1.0, 1.0);
            let color4 = graphene::Vec3::new(0.5, 0.0, 1.0);

            Self {
                shaders: Default::default(),
                pattern: RefCell::new(pattern),

                gradient_cache: TextureCache {
                    texture: RefCell::new(None),
                    params: RefCell::new(GradientCacheParams {
                        size: (0.0, 0.0),
                        colors: [color1, color2, color3, color4],
                        phase: 0,
                    }),
                },

                progress: Default::default(),
                phase: Default::default(),

                color1: Cell::new(color1),
                color2: Cell::new(color2),
                color3: Cell::new(color3),
                color4: Cell::new(color4),
                dark: Default::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GradientBackground {
        const NAME: &'static str = "ComponentsGradientBackground";
        type Type = super::GradientBackground;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for GradientBackground {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.set_vexpand(true);
            obj.set_hexpand(true);

            let target = adw::CallbackAnimationTarget::new(clone!(@weak obj => move |progress| {
                let imp = obj.imp();
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

            obj.connect_local("animate", true, move |_| {
                let val = animation.value();
                if val == 0.0 || val == 1.0 {
                    animation.play()
                }
                None
            });
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecBoxed::builder::<gdk::RGBA>("color1").build(),
                    glib::ParamSpecBoxed::builder::<gdk::RGBA>("color2").build(),
                    glib::ParamSpecBoxed::builder::<gdk::RGBA>("color3").build(),
                    glib::ParamSpecBoxed::builder::<gdk::RGBA>("color4").build(),
                    glib::ParamSpecBoolean::builder("dark").build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            if pspec.name() == "dark" {
                self.dark.set(value.get().unwrap());
            } else {
                let color: gdk::RGBA = value.get().unwrap();
                let color = graphene::Vec3::new(color.red(), color.green(), color.blue());
                match pspec.name() {
                    "color1" => self.color1.set(color),
                    "color2" => self.color2.set(color),
                    "color3" => self.color3.set(color),
                    "color4" => self.color4.set(color),
                    _ => unreachable!(),
                };
            }
            self.obj().queue_draw()
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            if pspec.name() == "dark" {
                self.dark.get().to_value()
            } else {
                let color = match pspec.name() {
                    "color1" => self.color1.get(),
                    "color2" => self.color2.get(),
                    "color3" => self.color3.get(),
                    "color4" => self.color4.get(),
                    _ => unreachable!(),
                };
                gdk::RGBA::new(color.x(), color.y(), color.z(), 1.0).to_value()
            }
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> =
                Lazy::new(|| vec![Signal::builder("animate").return_type::<()>().build()]);

            SIGNALS.as_ref()
        }
    }

    impl WidgetImpl for GradientBackground {
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            if let Some(child) = self.obj().first_child() {
                child.measure(orientation, for_size)
            } else {
                self.parent_measure(orientation, for_size)
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            if let Some(child) = self.obj().first_child() {
                child.allocate(width, height, baseline, None)
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            widget.ensure_shader();

            let Some(Some([_, pattern_shader])) = &*self.shaders.borrow() else {
                // fallback code
                if let Some(child) = widget.first_child() {
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

            // background
            if self.progress.get() == 0.0 {
                let texture = {
                    let params = GradientCacheParams {
                        size: (width, height),
                        colors: [
                            self.color1.get(),
                            self.color2.get(),
                            self.color3.get(),
                            self.color4.get(),
                        ],
                        phase: self.phase.get(),
                    };

                    if let Some(texture) = self.gradient_cache.try_get(params) {
                        texture
                    } else {
                        let renderer = self.obj().native().unwrap().renderer();
                        let texture = renderer
                            .render_texture(self.gradient_shader_node(&bounds), Some(&bounds));

                        self.gradient_cache.save(params, Some(texture.clone()));

                        texture
                    }
                };

                snapshot.append_texture(&texture, &bounds);
            } else {
                snapshot.append_node(self.gradient_shader_node(&bounds));
            }

            // pattern
            let args_builder = gsk::ShaderArgsBuilder::new(pattern_shader, None);

            args_builder.set_bool(0, self.dark.get());

            snapshot.push_gl_shader(pattern_shader, &bounds, args_builder.to_args());

            if let Some(child) = widget.first_child() {
                widget.snapshot_child(&child, snapshot);
            }
            snapshot.gl_shader_pop_texture();

            let pattern = self.pattern.borrow().clone();

            let pattern_bounds = graphene::Rect::new(
                0.0,
                0.0,
                pattern.width() as f32 * 0.3,
                pattern.height() as f32 * 0.3,
            );

            snapshot.push_repeat(&bounds, Some(&pattern_bounds));
            snapshot.append_texture(&pattern, &pattern_bounds);
            snapshot.pop();
            snapshot.gl_shader_pop_texture();

            snapshot.pop();
        }
    }

    impl GradientBackground {
        fn gradient_shader_node(&self, bounds: &graphene::Rect) -> gsk::GLShaderNode {
            let Some(Some([gradient_shader, _])) = &*self.shaders.borrow() else {
                panic!()
            };

            let args_builder = gsk::ShaderArgsBuilder::new(gradient_shader, None);

            args_builder.set_vec3(0, &self.color1.get());
            args_builder.set_vec3(1, &self.color2.get());
            args_builder.set_vec3(2, &self.color3.get());
            args_builder.set_vec3(3, &self.color4.get());

            let [p1, p2, p3, p4] =
                Self::calculate_positions(self.progress.get(), self.phase.get() as usize);

            args_builder.set_vec2(4, &p1);
            args_builder.set_vec2(5, &p2);
            args_builder.set_vec2(6, &p3);
            args_builder.set_vec2(7, &p4);

            gsk::GLShaderNode::new(gradient_shader, bounds, &args_builder.to_args(), &[])
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
    pub struct GradientBackground(ObjectSubclass<imp::GradientBackground>)
        @extends gtk::Widget;
}

impl GradientBackground {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn animate(&self) {
        self.emit_by_name::<()>("animate", &[]);
    }

    pub fn colors(&self) -> [gdk::RGBA; 4] {
        [
            self.property("color1"),
            self.property("color2"),
            self.property("color3"),
            self.property("color4"),
        ]
    }

    pub fn set_colors(&self, colors: [gdk::RGBA; 4]) {
        self.set_property("color1", colors[0]);
        self.set_property("color2", colors[1]);
        self.set_property("color3", colors[2]);
        self.set_property("color4", colors[3]);
    }

    pub fn is_dark(&self) -> bool {
        self.property("dark")
    }

    pub fn set_dark(&self, dark: bool) {
        self.set_property("dark", dark)
    }

    fn ensure_shader(&self) {
        let imp = self.imp();
        if imp.shaders.borrow().is_none() {
            let renderer = self.native().unwrap().renderer();

            let sources = [GRADIENT_SHADER, PATTERN_SHADER];

            let shaders: Vec<_> = sources
                .iter()
                .flat_map(|source| {
                    let bytes = glib::Bytes::from_static(source);
                    let shader = gsk::GLShader::from_bytes(&bytes);
                    if let Err(e) = shader.compile(&renderer) {
                        if e.message() != "The renderer does not support gl shaders" {
                            log::error!("can't compile shader for gradient background {e}");
                        }
                        return None;
                    }
                    Some(shader)
                })
                .collect();

            let shaders = shaders.try_into().ok();

            if shaders.is_none() {
                if let Some(c) = self.first_child() {
                    c.add_css_class("fallback")
                }
            }

            imp.shaders.replace(Some(shaders));
        }
    }
}

impl Default for GradientBackground {
    fn default() -> Self {
        Self::new()
    }
}
