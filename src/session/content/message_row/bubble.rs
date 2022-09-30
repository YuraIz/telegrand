use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{glib, CompositeTemplate};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::session::content::message_row::{MessageIndicators, MessageLabel};

const SENDER_CLASSES: &[&str] = &[
    "sender-text-red",
    "sender-text-orange",
    "sender-text-violet",
    "sender-text-green",
    "sender-text-cyan",
    "sender-text-blue",
    "sender-text-pink",
];

mod imp {
    use super::*;
    use once_cell::sync::Lazy;
    use std::cell::{Cell, RefCell};

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(string = r#"
    <interface>
      <template class="MessageBubble" parent="GtkWidget">
        <child>
          <object class="GtkOverlay" id="overlay">
            <child>
              <object class="GtkBox" id="content_box">
                <property name="orientation">vertical</property>
              </object>
            </child>
          </object>
        </child>
      </template>
    </interface>
    "#)]
    pub(crate) struct MessageBubble {
        pub(super) message_label: RefCell<Option<MessageLabel>>,
        pub(super) indicators: RefCell<Option<MessageIndicators>>,
        pub(super) sender_label: RefCell<Option<gtk::Label>>,
        pub(super) sender_id: Cell<i64>,
        pub(super) sender_color_class: RefCell<String>,
        pub(super) prefix: RefCell<Option<gtk::Widget>>,
        #[template_child]
        pub(super) overlay: TemplateChild<gtk::Overlay>,
        #[template_child]
        pub(super) content_box: TemplateChild<gtk::Box>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageBubble {
        const NAME: &'static str = "MessageBubble";
        type Type = super::MessageBubble;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.set_css_name("messagebubble");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessageBubble {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecString::builder("label")
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY)
                        .build(),
                    glib::ParamSpecObject::builder("indicators", MessageIndicators::static_type())
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY)
                        .build(),
                    glib::ParamSpecString::builder("sender")
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY)
                        .build(),
                    glib::ParamSpecInt64::builder("sender-id")
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY)
                        .build(),
                    glib::ParamSpecObject::builder("prefix", gtk::Widget::static_type())
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY)
                        .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(
            &self,
            obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "label" => obj.set_label(value.get().unwrap()),
                "indicators" => obj.set_indicators(value.get().unwrap()),
                "sender" => obj.set_sender(value.get().unwrap()),
                "sender-id" => obj.set_sender_id(value.get().unwrap()),
                "prefix" => obj.set_prefix(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "label" => obj.label().to_value(),
                "indicators" => obj.indicators().to_value(),
                "sender" => obj.sender().to_value(),
                "sender-id" => obj.sender_id().to_value(),
                "prefix" => obj.prefix().to_value(),
                _ => unimplemented!(),
            }
        }

        fn dispose(&self, _obj: &Self::Type) {
            self.overlay.unparent();
        }
    }

    impl WidgetImpl for MessageBubble {
        fn measure(
            &self,
            _widget: &Self::Type,
            orientation: gtk::Orientation,
            for_size: i32,
        ) -> (i32, i32, i32, i32) {
            if let Some(prefix) = self.prefix.borrow().as_ref() {
                if let gtk::Orientation::Horizontal = orientation {
                    let (minimum, mut natural, minimum_baseline, natural_baseline) =
                        self.overlay.measure(orientation, for_size);

                    // Manually set the default width of the widget to the one
                    // of the prefix
                    if for_size == -1 {
                        let (_, prefix_default_width, _, _) =
                            prefix.measure(gtk::Orientation::Horizontal, -1);
                        natural = prefix_default_width;
                    }

                    (minimum, natural, minimum_baseline, natural_baseline)
                } else {
                    self.overlay.measure(orientation, for_size)
                }
            } else {
                self.overlay.measure(orientation, for_size)
            }
        }

        fn size_allocate(&self, _widget: &Self::Type, width: i32, height: i32, baseline: i32) {
            self.overlay.allocate(width, height, baseline, None);
        }

        fn request_mode(&self, _widget: &Self::Type) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }
    }
}

glib::wrapper! {
    pub(crate) struct MessageBubble(ObjectSubclass<imp::MessageBubble>)
        @extends gtk::Widget;
}

impl MessageBubble {
    pub(crate) fn label(&self) -> String {
        self.imp()
            .message_label
            .borrow()
            .as_ref()
            .map(|l| l.label())
            .unwrap_or_default()
    }

    pub(crate) fn set_label(&self, label: String) {
        let imp = self.imp();

        if label.is_empty() {
            if let Some(message_label) = imp.message_label.take() {
                imp.content_box.remove(&message_label);

                // Make sure that the label is completely dropped and that
                // the indicators are also "unparented"
                drop(message_label);

                self.update_indicators_parent();
                self.remove_css_class("with-label");

                self.notify("label");
            }
        } else {
            let mut message_label_ref = imp.message_label.borrow_mut();

            if let Some(message_label) = &*message_label_ref {
                message_label.set_label(label);
            } else {
                let message_label = MessageLabel::new(&label, None);
                // TODO: connect notify signal
                imp.content_box.append(&message_label);

                *message_label_ref = Some(message_label);

                drop(message_label_ref);

                self.update_indicators_parent();
                self.add_css_class("with-label");

                self.notify("label");
            }
        }
    }

    pub(crate) fn indicators(&self) -> Option<MessageIndicators> {
        self.imp().indicators.borrow().clone()
    }

    pub(crate) fn set_indicators(&self, indicators: Option<MessageIndicators>) {
        let imp = self.imp();
        let old = imp.indicators.replace(indicators);
        if old != *imp.indicators.borrow() {
            self.update_indicators_parent();
            self.notify("indicators");
        }
    }

    pub(crate) fn sender(&self) -> String {
        self.imp()
            .sender_label
            .borrow()
            .as_ref()
            .map(|l| l.label().into())
            .unwrap_or_default()
    }

    pub(crate) fn set_sender(&self, sender: &str) {
        let imp = self.imp();

        if sender.is_empty() {
            if let Some(sender_label) = imp.sender_label.take() {
                imp.content_box.remove(&sender_label);
                self.notify("sender");
            }
        } else {
            let mut sender_label_ref = imp.sender_label.borrow_mut();

            if let Some(sender_label) = &*sender_label_ref {
                sender_label.set_label(sender);
            } else {
                let sender_label = gtk::Label::builder().label(sender).xalign(0.0).build();
                sender_label.add_css_class("sender-text");
                // TODO: connect notify signal
                imp.content_box.prepend(&sender_label);

                *sender_label_ref = Some(sender_label);

                drop(sender_label_ref);

                self.update_sender_color();

                self.notify("sender");
            }
        }
    }

    pub(crate) fn sender_id(&self) -> i64 {
        self.imp().sender_id.get()
    }

    pub(crate) fn set_sender_id(&self, sender_id: i64) {
        let imp = self.imp();
        let old = imp.sender_id.replace(sender_id);
        if old != sender_id {
            self.update_sender_color();
            self.notify("sender-id");
        }
    }

    pub(crate) fn prefix(&self) -> Option<gtk::Widget> {
        self.imp().prefix.borrow().clone()
    }

    pub(crate) fn set_prefix(&self, prefix: Option<gtk::Widget>) {
        let imp = self.imp();
        let old = imp.prefix.replace(prefix);
        if old != *imp.prefix.borrow() {
            if let Some(old) = old {
                imp.content_box.remove(&old);
            }

            if let Some(prefix) = imp.prefix.borrow().as_ref() {
                imp.content_box.prepend(prefix);
            }

            self.update_indicators_parent();

            self.notify("prefix");
        }
    }

    fn update_sender_color(&self) {
        let imp = self.imp();

        if let Some(sender_label) = imp.sender_label.borrow().as_ref() {
            let sender_id = imp.sender_id.get();
            let color_class = SENDER_CLASSES[if sender_id != 0 {
                sender_id as usize
            } else {
                let mut s = DefaultHasher::new();
                sender_label.label().hash(&mut s);
                s.finish() as usize
            } % SENDER_CLASSES.len()];

            let mut sender_color_class = imp.sender_color_class.borrow_mut();
            let old_color_class = &*sender_color_class;
            if old_color_class != color_class {
                if !old_color_class.is_empty() {
                    sender_label.remove_css_class(old_color_class);
                }

                sender_label.add_css_class(color_class);
                *sender_color_class = color_class.into();
            }
        }
    }

    fn update_indicators_parent(&self) {
        let imp = self.imp();

        if let Some(indicators) = imp.indicators.borrow().as_ref() {
            if let Some(message_label) = imp.message_label.borrow().as_ref() {
                if indicators.parent() == Some(imp.overlay.clone().upcast()) {
                    imp.overlay.remove_overlay(indicators);
                }

                message_label.set_indicators(Some(indicators.clone()));
            } else if indicators.parent() != Some(imp.overlay.clone().upcast()) {
                imp.overlay.add_overlay(indicators);
            }
        }
    }
}
