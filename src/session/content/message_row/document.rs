use glib::{clone, format_size};
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib, CompositeTemplate};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tdlib::enums::MessageContent;
use tdlib::types::File;

use crate::session::content::message_row::{
    MessageBase, MessageBaseImpl, MessageIndicators, MessageLabel,
};
use crate::tdlib::{ChatType, Message, MessageSender};
use crate::utils::parse_formatted_text;
use crate::Session;

use super::base::MessageBaseExt;

mod imp {
    use super::*;
    use once_cell::sync::Lazy;
    use std::cell::RefCell;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/melix99/telegrand/ui/content-message-document.ui")]
    pub(crate) struct MessageDocument {
        pub(super) sender_color_class: RefCell<Option<String>>,
        pub(super) bindings: RefCell<Vec<gtk::ExpressionWatch>>,
        pub(super) message: RefCell<Option<glib::Object>>,
        #[template_child]
        pub(super) sender_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) content_label: TemplateChild<MessageLabel>,
        #[template_child]
        pub(super) indicators: TemplateChild<MessageIndicators>,
        #[template_child]
        pub(super) file_name: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) file_size: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) file_status: TemplateChild<gtk::Button>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageDocument {
        const NAME: &'static str = "ContentMessageDocument";
        type Type = super::MessageDocument;
        type ParentType = MessageBase;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.set_css_name("messagedocument");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessageDocument {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecObject::new(
                    "message",
                    "Message",
                    "The message represented by this row",
                    glib::Object::static_type(),
                    glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                )]
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
                "message" => obj.set_message(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "message" => self.message.borrow().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for MessageDocument {}
    impl MessageBaseImpl for MessageDocument {}
}

glib::wrapper! {
    pub(crate) struct MessageDocument(ObjectSubclass<imp::MessageDocument>)
        @extends gtk::Widget, MessageBase;
}

impl MessageBaseExt for MessageDocument {
    type Message = glib::Object;

    fn set_message(&self, message: Self::Message) {
        let imp = self.imp();

        if imp.message.borrow().as_ref() == Some(&message) {
            return;
        }

        let mut bindings = imp.bindings.borrow_mut();

        while let Some(binding) = bindings.pop() {
            binding.unwatch();
        }

        imp.indicators.set_message(message.clone());

        // Remove the previous color css class
        let mut sender_color_class = imp.sender_color_class.borrow_mut();
        if let Some(class) = sender_color_class.as_ref() {
            imp.sender_label.remove_css_class(class);
            *sender_color_class = None;
        }

        if let Some(message) = message.downcast_ref::<Message>() {
            // Show sender label, if needed
            let show_sender = if message.chat().is_own_chat() {
                if message.is_outgoing() {
                    None
                } else {
                    Some(message.forward_info().unwrap().origin().id())
                }
            } else if message.is_outgoing() {
                if matches!(message.sender(), MessageSender::Chat(_)) {
                    Some(Some(message.sender().id()))
                } else {
                    None
                }
            } else if matches!(
                message.chat().type_(),
                ChatType::BasicGroup(_) | ChatType::Supergroup(_)
            ) {
                Some(Some(message.sender().id()))
            } else {
                None
            };

            if let Some(maybe_id) = show_sender {
                let sender_name_expression = message.sender_display_name_expression();
                let sender_binding =
                    sender_name_expression.bind(&*imp.sender_label, "label", glib::Object::NONE);
                bindings.push(sender_binding);

                // Color sender label
                let classes = vec![
                    "sender-text-red",
                    "sender-text-orange",
                    "sender-text-violet",
                    "sender-text-green",
                    "sender-text-cyan",
                    "sender-text-blue",
                    "sender-text-pink",
                ];

                let color_class = classes[maybe_id.map(|id| id as usize).unwrap_or_else(|| {
                    let mut s = DefaultHasher::new();
                    imp.sender_label.label().hash(&mut s);
                    s.finish() as usize
                }) % classes.len()];
                imp.sender_label.add_css_class(color_class);

                *sender_color_class = Some(color_class.into());

                imp.sender_label.set_visible(true);
            } else {
                imp.sender_label.set_visible(false);
            }

            self.update_document(message);
        } else {
            unreachable!("Unexpected message type: {:?}", message);
        }

        imp.message.replace(Some(message));
        self.notify("message");
    }
}

impl MessageDocument {
    fn update_document(&self, message: &Message) {
        if let MessageContent::MessageDocument(data) = message.content().0 {
            let imp = self.imp();

            let message_text = parse_formatted_text(data.caption);
            imp.content_label.set_visible(!message_text.is_empty());
            imp.content_label.set_label(message_text);

            imp.file_name.set_label(&data.document.file_name);

            let session = message.chat().session();

            self.update_status(data.document.document, session);
        }
    }

    fn update_status(&self, document: File, session: Session) {
        let local = &document.local;
        let imp = self.imp();

        if local.is_downloading_completed {
            imp.file_status.set_icon_name("folder-documents-symbolic");
            imp.file_size
                .set_label(&format_size(local.downloaded_size as u64));

            let path = local.path.clone();
            let gio_file = gio::File::for_path(&path);

            imp.file_status.connect_clicked(move |_| {
                gio::AppInfo::launch_default_for_uri(
                    &gio_file.uri(),
                    Option::<&gio::AppLaunchContext>::None,
                )
                .ok();
            });
        } else if !local.is_downloading_active {
            imp.file_status.set_icon_name("document-save-symbolic");

            let file_size = if document.size != 0 {
                document.size as u64
            } else {
                document.expected_size as u64
            };

            imp.file_size.set_label(&format_size(file_size));

            let id = document.id;

            imp.file_status.connect_clicked(
                clone!(@weak self as obj, @strong session => move |file_status| {

                    // I don't know how to cancel downloading
                    file_status.set_icon_name("media-playback-stop-symbolic");
                    file_status.connect_clicked(|_| {});

                    let (sender, receiver) = glib::MainContext::sync_channel::<File>(Default::default(), 5);
                    receiver.attach(
                        None,
                        clone!(@weak obj, @weak session => @default-return glib::Continue(false), move |file| {
                            if file.local.is_downloading_completed {
                                obj.update_status(file, session);
                                glib::Continue(false)
                            } else {
                                let progress = file.local.downloaded_size as f64 / file.expected_size as f64;

                                obj.imp().file_size.set_label(
                                    format!(
                                        "{:.0}% of {}",
                                        progress * 100.0,
                                        format_size(document.expected_size as u64)
                                    )
                                    .as_str(),
                                );
                                glib::Continue(true)
                            }
                        }));

                    session.download_file(id, sender);
                }),
            );
        }
    }
}
