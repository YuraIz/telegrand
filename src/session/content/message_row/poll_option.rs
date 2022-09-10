#![allow(unused)]

use glib::{clone, format_size};
use gtk::glib::SignalHandlerId;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib, CompositeTemplate};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tdlib::enums::MessageContent;
use tdlib::enums::PollType;
use tdlib::types::File;

use crate::tdlib::{ChatType, Message, MessageSender};
use crate::utils::parse_formatted_text;
use crate::Session;

mod imp {
    use gtk::TemplateChild;

    use super::*;

    use crate::session::content::ChatHistory;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(string = r#"
        <interface>
          <template class="ContentPollOption" parent="GtkButton">
            <style>
              <class name="flat" />
            </style>
            <child>
              <object class="GtkBox">
                <property name="orientation">vertical</property>
                <child>
                  <object class="GtkBox">
                    <property name="orientation">horizontal</property>
                    <child>
                      <object class="GtkCheckButton">
                        </object>
                    </child>
                    <child>
                      <object class="GtkLabel" id="label">
                        </object>
                    </child>
                  </object>
                </child>
                <child>
                  <object class="GtkProgressBar" id="percentage">
                    </object>
                </child>
                <child>
                  <object class="GtkSeparator">
                    <property name="orientation">vertical</property>
                  </object>
                </child>
              </object>
            </child>
          </template>
        </interface>
    "#)]
    pub(crate) struct PollOption {
        #[template_child]
        pub(crate) label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(crate) percentage: TemplateChild<gtk::ProgressBar>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PollOption {
        const NAME: &'static str = "ContentPollOption";
        // const ABSTRACT: bool = true;
        type Type = super::PollOption;
        type ParentType = gtk::Button;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PollOption {}

    impl WidgetImpl for PollOption {}

    impl ButtonImpl for PollOption {}
}

glib::wrapper! {
    pub(crate) struct PollOption(ObjectSubclass<imp::PollOption>)
        @extends gtk::Button, gtk::Widget;
}

impl PollOption {
    pub(crate) fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create PollOption")
    }
}

impl From<&tdlib::types::PollOption> for PollOption {
    fn from(option: &tdlib::types::PollOption) -> Self {
        let obj = Self::new();

        let imp = obj.imp();

        imp.label.set_label(&option.text);

        let fraction = option.vote_percentage as f64 / 100.0;

        imp.percentage.set_fraction(fraction);

        // option.

        // let gtk_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        // let check_button = gtk::CheckButton::new();
        // check_button.set_group(Some(&group));

        // let label = gtk::Label::new(Some(&option.text));

        // gtk_box.append(&check_button);
        // gtk_box.append(&label);

        obj
    }
}
