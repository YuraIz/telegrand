use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{
    gdk,
    glib::{self, WeakRef},
    graphene,
};

#[derive(Debug, Clone)]
struct RltOverlayChild {
    animation: fixed_size::Fixed,
    target: WeakRef<gtk::Widget>,
    shift: (i32, i32),
}

impl RltOverlayChild {
    fn into_variant(self) -> glib::Variant {
        let ptr = Box::into_raw(Box::new(self)) as u64;
        ptr.to_variant()
    }
}

impl glib::FromVariant for RltOverlayChild {
    fn from_variant(variant: &glib::Variant) -> Option<Self> {
        let ptr: u64 = variant.get()?;
        unsafe { Some(*Box::from_raw(ptr as *mut _)) }
    }
}

impl glib::StaticVariantType for RltOverlayChild {
    fn static_variant_type() -> std::borrow::Cow<'static, glib::VariantTy> {
        u64::static_variant_type()
    }
}

mod fixed_size {
    use super::*;
    use glib::Object;
    use gtk::glib;

    mod imp {
        use glib::once_cell::sync::OnceCell;
        use std::cell::Cell;

        use super::*;

        #[derive(Default)]
        pub struct Fixed {
            pub(super) child: OnceCell<rlt::Animation>,
            pub(super) size: Cell<i32>,
        }

        #[glib::object_subclass]
        impl ObjectSubclass for Fixed {
            const NAME: &'static str = "RltFixed";
            type ParentType = gtk::Widget;
            type Type = super::Fixed;
        }

        impl ObjectImpl for Fixed {
            fn constructed(&self) {
                let obj = self.obj();

                obj.set_halign(gtk::Align::Start);
                obj.set_valign(gtk::Align::Start);
            }
        }
        impl WidgetImpl for Fixed {
            fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
                self.child
                    .get()
                    .unwrap()
                    .allocate(width, height, baseline, None);
            }

            fn request_mode(&self) -> gtk::SizeRequestMode {
                gtk::SizeRequestMode::ConstantSize
            }

            fn measure(&self, _: gtk::Orientation, _: i32) -> (i32, i32, i32, i32) {
                (0, self.size.get(), -1, -1)
            }
        }

        impl Drop for Fixed {
            fn drop(&mut self) {
                self.child.get().unwrap().unparent();
            }
        }
    }

    glib::wrapper! {
        pub struct Fixed(ObjectSubclass<imp::Fixed>)
            @extends gtk::Widget;
    }

    impl Fixed {
        pub fn new(animation: &rlt::Animation, size: i32) -> Self {
            let obj: Self = Object::new(&[]);
            let imp = obj.imp();
            animation.set_parent(&obj);
            imp.child.set(animation.clone()).unwrap();
            imp.size.set(size);
            obj
        }
    }

    impl std::ops::Deref for Fixed {
        type Target = rlt::Animation;

        fn deref(&self) -> &Self::Target {
            self.imp().child.get().unwrap().as_ref()
        }
    }
}

mod imp {
    use std::cell::{Cell, RefCell};

    use glib::subclass::Signal;
    use glib::{clone, WeakRef};
    use once_cell::sync::Lazy;
    use once_cell::unsync::OnceCell;

    use super::*;

    #[derive(Debug, Default)]
    pub struct RltOverlay {
        pub(super) children: RefCell<Vec<RltOverlayChild>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RltOverlay {
        const NAME: &'static str = "ComponentsRltOverlay";
        type Type = super::RltOverlay;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.install_action(
                "rlt-overlay.append",
                Some(RltOverlayChild::static_variant_type().as_str()),
                |widget, _, variant| {
                    let child: RltOverlayChild = variant.and_then(|v| v.get()).unwrap();
                    child.animation.set_sensitive(false);
                    child.animation.set_parent(widget);
                    widget.imp().children.borrow_mut().push(child);
                },
            );
        }
    }

    impl ObjectImpl for RltOverlay {}

    impl WidgetImpl for RltOverlay {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let obj = self.obj();

            if let Some(ref child) = obj.child() {
                obj.snapshot_child(child, snapshot)
            }

            for child in &*self.children.borrow() {
                obj.snapshot_animation(child, snapshot)
            }

            obj.drop_inactive_animations();
        }
    }

    impl BinImpl for RltOverlay {}
}

glib::wrapper! {
    pub struct RltOverlay(ObjectSubclass<imp::RltOverlay>)
        @extends gtk::Widget, adw::Bin;
}

impl RltOverlay {
    pub fn new() -> Self {
        glib::Object::new(&[])
    }

    pub fn append<T: IsA<gtk::Widget>>(
        target: &T,
        animation: rlt::Animation,
        size: i32,
        shift_x: i32,
        shift_y: i32,
    ) {
        animation.set_loop(false);
        animation.play();

        let animation = fixed_size::Fixed::new(&animation, size);

        let child = RltOverlayChild {
            animation,
            target: target.upcast_ref().downgrade(),
            shift: (shift_x, shift_y),
        }
        .into_variant();

        target
            .activate_action("rlt-overlay.append", Some(&child))
            .unwrap()
    }

    fn snapshot_animation(&self, child: &RltOverlayChild, snapshot: &gtk::Snapshot) {
        if let Some(parent) = child.target.upgrade() {
            let opacity = child.animation.opacity();

            if !parent.is_mapped() {
                if opacity <= 0.0 {
                    return;
                }
                child.animation.set_opacity(opacity - 0.2)
            } else if opacity < 1.0 {
                child.animation.set_opacity(opacity + 0.2)
            }

            let Some(bounds) = parent.compute_bounds(self) else {return};

            let center = bounds.center();

            let shift_x = child.animation.width() as f32 * 0.5 + child.shift.0 as f32;
            let shift_y = child.animation.width() as f32 * 0.5 + child.shift.1 as f32;

            let x = center.x() - shift_x;
            let y = center.y() - shift_y;

            snapshot.translate(&graphene::Point::new(x, y));
            self.snapshot_child(&child.animation, snapshot);
            snapshot.translate(&graphene::Point::new(-x, -y));
        }
    }

    fn drop_inactive_animations(&self) {
        let mut children = self.imp().children.borrow_mut();

        let mut i = 0;
        while i < children.len() {
            let child = &children[i];

            let ended = !child.animation.is_playing();

            if ended {
                let child = children.remove(i);
                child.animation.unparent();
            } else {
                i += 1;
            }
        }
    }
}

impl Default for RltOverlay {
    fn default() -> Self {
        Self::new()
    }
}
