use glib::{clone, format_size};
use gtk::gdk;
use gtk::glib::SignalHandlerId;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{cairo, gio, glib, CompositeTemplate};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

mod imp {
    use super::*;
    use glib::types::StaticType;
    use once_cell::sync::Lazy;
    use std::cell::Cell;
    use std::cell::RefCell;
    use std::f64::consts::PI as M_PI;

    #[derive(Debug, Default)]
    pub(crate) struct ProgressCircle {
        pub(super) progress: Cell<f64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ProgressCircle {
        const NAME: &'static str = "ContentProgressCircle";
        type Type = super::ProgressCircle;
        type ParentType = gtk::DrawingArea;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("progresscircle");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            // obj.init_template();
        }
    }

    impl ProgressCircle {
        fn draw(
            drawing_area: &gtk::DrawingArea,
            context: &cairo::Context,
            width: i32,
            height: i32,
        ) {
            let progress_circle = drawing_area
                .downcast_ref::<super::ProgressCircle>()
                .unwrap();

            let color = progress_circle.style_context().color();
            let progress = progress_circle.progress();

            let xc = width as f64 / 2.0;
            let yc = height as f64 / 2.0;
            let radius = xc.min(yc) - 4.0;

            let angle1 = -100.0 * (M_PI / 180.0); /* angles are specified */
            let angle2 = angle1 + 20.0 / 180.0 * M_PI + 340.0 / 180.0 * M_PI * progress; /* in radians           */
            // let angle2 = 90. * (M_PI / 180.0); /* angles are specified */
            context.set_source_rgba(
                color.red() as f64,
                color.green() as f64,
                color.blue() as f64,
                color.alpha() as f64,
            );

            context.set_line_width(4.0);
            context.arc(xc, yc, radius, angle1, angle2);
            context.stroke();
        }
    }

    impl ObjectImpl for ProgressCircle {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);
            obj.set_draw_func(Self::draw);
        }
    }

    impl WidgetImpl for ProgressCircle {}
    impl DrawingAreaImpl for ProgressCircle {}
}

glib::wrapper! {
    pub(crate) struct ProgressCircle(ObjectSubclass<imp::ProgressCircle>)
        @extends gtk::DrawingArea, gtk::Widget;
}

impl ProgressCircle {
    pub(crate) fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create `ContentProgressCircle`.")
    }

    pub fn set_progress(&self, progress: f64) {
        self.imp().progress.set(progress);
        self.queue_draw()
    }

    fn progress(&self) -> f64 {
        self.imp().progress.get()
    }
}
