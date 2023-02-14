use gettextrs::gettext;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{glib, CompositeTemplate};

use crate::tdlib::{Chat, User};

mod imp {
    use super::*;
    use once_cell::sync::Lazy;
    use std::cell::RefCell;

    use crate::components::Avatar;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(string = r#"
    template SidebarSearchItemRow {
        .ComponentsAvatar avatar {
            size: 32;
            item: bind SidebarSearchItemRow.item;
        }

        Inscription label {
            hexpand: true;
            text-overflow: ellipsize_end;
        }
    }
    "#)]
    pub(crate) struct ItemRow {
        /// A `Chat` or `User`
        pub(super) item: RefCell<Option<glib::Object>>,
        #[template_child]
        pub(super) avatar: TemplateChild<Avatar>,
        #[template_child]
        pub(super) label: TemplateChild<gtk::Inscription>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ItemRow {
        const NAME: &'static str = "SidebarSearchItemRow";
        type Type = super::ItemRow;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.set_css_name("sidebarsearchitemrow");
            klass.set_layout_manager_type::<gtk::BoxLayout>();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ItemRow {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecObject::builder::<glib::Object>("item")
                    .explicit_notify()
                    .build()]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();

            match pspec.name() {
                "item" => obj.set_item(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();

            match pspec.name() {
                "item" => obj.item().to_value(),
                _ => unimplemented!(),
            }
        }

        fn dispose(&self) {
            self.avatar.unparent();
            self.label.unparent();
        }
    }

    impl WidgetImpl for ItemRow {}
}

glib::wrapper! {
    pub(crate) struct ItemRow(ObjectSubclass<imp::ItemRow>)
        @extends gtk::Widget;
}

impl ItemRow {
    pub(crate) fn new(item: &Option<glib::Object>) -> Self {
        glib::Object::builder().property("item", item).build()
    }

    pub(crate) fn set_item(&self, item: Option<glib::Object>) {
        if self.item() == item {
            return;
        }

        let imp = self.imp();

        if let Some(chat) = item.as_ref().and_then(|i| i.downcast_ref::<Chat>()) {
            if chat.is_own_chat() {
                imp.label.set_text(Some(&gettext("Saved Messages")));
            } else {
                imp.label.set_text(Some(&chat.title()));
            }
        } else if let Some(user) = item.as_ref().and_then(|i| i.downcast_ref::<User>()) {
            imp.label
                .set_text(Some(&(user.first_name() + " " + &user.last_name())));
        } else {
            imp.label.set_text(Some(""));

            if let Some(ref item) = item {
                log::warn!("Unexpected item type {:?}", item);
            }
        }

        imp.item.replace(item);
        self.notify("item");
    }

    pub(crate) fn item(&self) -> Option<glib::Object> {
        self.imp().item.borrow().clone()
    }
}
