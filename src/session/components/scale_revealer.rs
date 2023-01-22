use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, glib, graphene};

const ANIMATION_DURATION: u32 = 250;

mod imp {
    use std::cell::{Cell, RefCell};

    use glib::subclass::Signal;
    use glib::{clone, WeakRef};
    use once_cell::sync::Lazy;
    use once_cell::unsync::OnceCell;

    use super::*;

    #[derive(Debug, Default)]
    pub struct ScaleRevealer {
        pub reveal_child: Cell<bool>,
        pub source_widget: WeakRef<gtk::Widget>,
        pub source_widget_texture: RefCell<Option<gdk::Texture>>,
        pub animation: OnceCell<adw::TimedAnimation>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ScaleRevealer {
        const NAME: &'static str = "ComponentsScaleRevealer";
        type Type = super::ScaleRevealer;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for ScaleRevealer {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> =
                Lazy::new(|| vec![Signal::builder("transition-done").build()]);
            SIGNALS.as_ref()
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecBoolean::builder("reveal-child")
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecObject::builder::<gtk::Widget>("source-widget")
                        .explicit_notify()
                        .build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "reveal-child" => self.obj().set_reveal_child(value.get().unwrap()),
                "source-widget" => self
                    .obj()
                    .set_source_widget(value.get::<Option<&gtk::Widget>>().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "reveal-child" => self.obj().reveals_child().to_value(),
                "source-widget" => self.obj().source_widget().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            let target = adw::CallbackAnimationTarget::new(clone!(@weak obj => move |_| {
                obj.queue_draw();
            }));
            let animation = adw::TimedAnimation::new(&*obj, 0.0, 1.0, ANIMATION_DURATION, &target);

            animation.set_easing(adw::Easing::EaseOutQuart);
            animation.connect_done(clone!(@weak obj => move |_| {
                let imp = obj.imp();

                if !imp.reveal_child.get() {
                    if let Some(source_widget) = imp.source_widget.upgrade() {
                        // Show the original source widget now that the
                        // transition is over.
                        source_widget.set_opacity(1.0);
                    }
                    obj.set_visible(false);
                }

                obj.emit_by_name::<()>("transition-done", &[]);
            }));

            self.animation.set(animation).unwrap();
            obj.set_visible(false);
        }
    }

    impl WidgetImpl for ScaleRevealer {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let obj = self.obj();
            if let Some(child) = obj.child() {
                let progress = self.animation.get().unwrap().value() as f32;
                if progress == 1.0 {
                    // The transition progress is at 100%, so just show the child
                    obj.snapshot_child(&child, snapshot);
                    return;
                }

                let source_bounds = self
                    .source_widget
                    .upgrade()
                    .and_then(|s| s.compute_bounds(&*obj))
                    .unwrap_or_else(|| {
                        log::warn!(
                            "The source widget bounds could not be calculated, using default bounds as fallback"
                        );
                        graphene::Rect::new(0.0, 0.0, 100.0, 100.0)
                    });

                let interpolate = |start: f32, end: f32| start + (end - start) * progress;

                let obj_width = obj.width() as f32;
                let obj_height = obj.height() as f32;

                let (fin_width, fin_height) = fit_rect(
                    source_bounds.width(),
                    source_bounds.height(),
                    obj_width,
                    obj_height,
                );

                let fin_x = (obj_width - fin_width) * 0.5;
                let fin_y = (obj_height - fin_height) * 0.5;

                let x = interpolate(source_bounds.x(), fin_x);
                let y = interpolate(source_bounds.y(), fin_y);

                let width = interpolate(source_bounds.width(), fin_width);
                let height = interpolate(source_bounds.height(), fin_height);

                let scale = (width / obj_width).max(height / obj_height);

                snapshot.translate(&graphene::Point::new(x, y));
                snapshot.scale(scale, scale);

                if let Some(texture) = &*self.source_widget_texture.borrow() {
                    snapshot.push_cross_fade(progress as f64);
                    texture.snapshot(snapshot, fin_width as _, fin_height as _);
                    snapshot.pop();

                    snapshot.translate(&graphene::Point::new(-fin_x, -fin_y));
                    obj.snapshot_child(&child, snapshot);
                    snapshot.pop();
                } else {
                    log::warn!(
                        "The source widget texture is None, using child snapshot as fallback"
                    );
                    obj.snapshot_child(&child, snapshot);
                }
            }
        }
    }

    impl BinImpl for ScaleRevealer {}
}

glib::wrapper! {
    pub struct ScaleRevealer(ObjectSubclass<imp::ScaleRevealer>)
        @extends gtk::Widget, adw::Bin;
}

impl ScaleRevealer {
    pub fn new() -> Self {
        glib::Object::new(&[])
    }

    /// Whether the child is revealed or not.
    pub fn reveals_child(&self) -> bool {
        self.imp().reveal_child.get()
    }

    /// Set whether the child should be revealed or not.
    ///
    /// This will start the scale animation.
    pub fn set_reveal_child(&self, reveal_child: bool) {
        if self.reveals_child() == reveal_child {
            return;
        }

        let imp = self.imp();
        let animation = imp.animation.get().unwrap();
        animation.set_value_from(animation.value());

        if reveal_child {
            animation.set_value_to(1.0);
            self.set_visible(true);

            if let Some(source_widget) = imp.source_widget.upgrade() {
                // Render the current state of the source widget to a texture.
                // This will be needed for the transition.
                let texture = render_widget_to_texture(&source_widget);
                imp.source_widget_texture.replace(texture);

                // Hide the source widget.
                // We use opacity here so that the widget will stay allocated.
                source_widget.set_opacity(0.0);
            } else {
                imp.source_widget_texture.replace(None);
            }
        } else {
            animation.set_value_to(0.0);
        }

        imp.reveal_child.set(reveal_child);

        animation.play();

        self.notify("reveal-child");
    }

    /// The source widget this revealer is transitioning from.
    pub fn source_widget(&self) -> Option<gtk::Widget> {
        self.imp().source_widget.upgrade()
    }

    /// Set the source widget this revealer should transition from to show the
    /// child.
    pub fn set_source_widget(&self, source_widget: Option<&impl IsA<gtk::Widget>>) {
        let source_widget = source_widget.map(|s| s.as_ref());
        if self.source_widget().as_ref() == source_widget {
            return;
        }
        self.imp().source_widget.set(source_widget);
        self.notify("source-widget");
    }

    pub fn connect_transition_done<F: Fn(&Self) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("transition-done", true, move |values| {
            let obj = values[0].get::<Self>().unwrap();
            f(&obj);
            None
        })
    }
}

impl Default for ScaleRevealer {
    fn default() -> Self {
        Self::new()
    }
}

fn render_widget_to_texture(widget: &impl IsA<gtk::Widget>) -> Option<gdk::Texture> {
    let widget_paintable = gtk::WidgetPaintable::new(Some(widget.as_ref()));
    let snapshot = gtk::Snapshot::new();

    widget_paintable.snapshot(
        &snapshot,
        widget_paintable.intrinsic_width() as f64,
        widget_paintable.intrinsic_height() as f64,
    );

    let node = snapshot.to_node()?;
    let native = widget.native()?;

    Some(native.renderer().render_texture(node, None))
}

fn fit_rect(inner_width: f32, inner_height: f32, width: f32, height: f32) -> (f32, f32) {
    let inner_ar = inner_width / inner_height;
    let aspect_ratio = width / height;

    if inner_ar > aspect_ratio {
        (width, height * aspect_ratio / inner_ar)
    } else {
        (width / aspect_ratio * inner_ar, height)
    }
}
