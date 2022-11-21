use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

mod imp {
    use super::*;
    use gtk::graphene;
    use std::cell::Cell;

    #[derive(Default)]
    pub struct LoadingIndicator(pub(super) Cell<f64>);

    #[glib::object_subclass]
    impl ObjectSubclass for LoadingIndicator {
        const NAME: &'static str = "ComponentsLoadingIndicator";
        type Type = super::LoadingIndicator;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("loadingindicator");
        }
    }

    impl ObjectImpl for LoadingIndicator {}

    impl WidgetImpl for LoadingIndicator {
        fn measure(
            &self,
            _widget: &Self::Type,
            _orientation: gtk::Orientation,
            for_size: i32,
        ) -> (i32, i32, i32, i32) {
            (0, for_size, 0, 0)
        }

        fn snapshot(&self, widget: &Self::Type, snapshot: &gtk::Snapshot) {
            let size = widget.width() as f32;
            let bounds = graphene::Rect::new(0.0, 0.0, size, size);
            let context = snapshot.append_cairo(&bounds);
            let color = widget.style_context().color();
            context.set_source_rgba(
                color.red() as _,
                color.green() as _,
                color.blue() as _,
                color.alpha() as _,
            );
            let half_size = size as f64 / 2.0;

            let pi = std::f64::consts::PI;

            context.set_line_width(2.0);

            let start = -0.5 * pi;
            let diff = self.0.get() * 2.0 * pi;

            context.arc(half_size, half_size, half_size - 4.0, start, start + diff);
            context.stroke().unwrap();
        }
    }
}

glib::wrapper! {
    pub struct LoadingIndicator(ObjectSubclass<imp::LoadingIndicator>)
        @extends gtk::Widget;
}

impl LoadingIndicator {
    pub fn set_progress(&self, progress: f64) {
        let current = self.imp().0.get();
        if (current - progress).abs() > 1e-3 {
            self.imp().0.set(progress);
            self.queue_draw();
        }
    }
}
