use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use tdlib::types::ClosedVectorPath;

mod imp {
    use super::*;
    use gtk::graphene;
    use std::cell::RefCell;
    use tdlib::enums::VectorPathCommand::{CubicBezierCurve, Line};
    use tdlib::types::VectorPathCommandCubicBezierCurve as Curve;

    #[derive(Default)]
    pub struct StickerPreview {
        pub(super) path: RefCell<Vec<ClosedVectorPath>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for StickerPreview {
        const NAME: &'static str = "ComponentsVectorPath";
        type Type = super::StickerPreview;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for StickerPreview {}

    impl WidgetImpl for StickerPreview {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();

            let context = snapshot.append_cairo(&graphene::Rect::new(0.0, 0.0, 512.0, 512.0));

            let scale = widget.width().max(widget.height()) as f64 / 512.0;
            context.scale(scale, scale);

            context.set_source_rgba(0.5, 0.5, 0.5, 0.4);

            let outline = &*self.path.borrow();

            for closed_path in outline {
                for command in &closed_path.commands {
                    match command {
                        Line(line) => {
                            let e = &line.end_point;
                            context.line_to(e.x, e.y);
                        }
                        CubicBezierCurve(curve) => {
                            let Curve {
                                start_control_point: sc,
                                end_control_point: ec,
                                end_point: e,
                            } = curve;

                            context.curve_to(sc.x, sc.y, ec.x, ec.y, e.x, e.y);
                        }
                    }
                }
                _ = context.fill();
            }
        }
    }
}

glib::wrapper! {
    pub struct StickerPreview(ObjectSubclass<imp::StickerPreview>)
        @extends gtk::Widget;
}

impl StickerPreview {
    pub fn new(outline: Vec<ClosedVectorPath>) -> Self {
        let obj: Self = glib::Object::new(&[]);
        obj.imp().path.replace(outline);
        obj
    }
}
