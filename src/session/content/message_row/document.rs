use glib::{clone, closure};
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
use crate::tdlib::{BoxedMessageContent, ChatType, Message, MessageSender};
use crate::utils::parse_formatted_text;
use crate::Session;

use super::base::MessageBaseExt;

mod imp {
    use crate::session::content::message_row::bubble::MessageBubble;

    use super::*;
    use once_cell::sync::Lazy;
    use std::cell::RefCell;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/melix99/telegrand/ui/content-message-document.ui")]
    pub(crate) struct MessageDocument {
        pub(super) sender_color_class: RefCell<Option<String>>,
        pub(super) bindings: RefCell<Vec<gtk::ExpressionWatch>>,
        pub(super) handler_id: RefCell<Option<glib::SignalHandlerId>>,
        pub(super) status_handler_id: RefCell<Option<glib::SignalHandlerId>>,
        pub(super) message: RefCell<Option<Message>>,
        #[template_child]
        pub(super) document_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) click: TemplateChild<gtk::GestureClick>,
        #[template_child]
        pub(super) file_status_image: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) file_name_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) file_size_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) indicators: TemplateChild<MessageIndicators>,
        #[template_child]
        pub(super) message_bubble: TemplateChild<MessageBubble>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageDocument {
        const NAME: &'static str = "ContentMessageDocument";
        type Type = super::MessageDocument;
        type ParentType = MessageBase;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.set_css_name("messagedocument");
            klass.set_layout_manager_type::<gtk::BinLayout>();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessageDocument {
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
    type Message = Message;

    fn set_message(&self, message: Self::Message) {
        let imp = self.imp();

        if imp.message.borrow().as_ref() == Some(&message) {
            return;
        }

        let mut bindings = imp.bindings.borrow_mut();

        while let Some(binding) = bindings.pop() {
            binding.unwatch();
        }

        imp.indicators.set_message(message.clone().upcast());

        // Show sender label, if needed
        let show_sender = if message.chat().is_own_chat() {
            if message.is_outgoing() {
                None
            } else {
                Some(
                    message
                        .forward_info()
                        .unwrap()
                        .origin()
                        .id()
                        .unwrap_or_default(),
                )
            }
        } else if message.is_outgoing() {
            if matches!(message.sender(), MessageSender::Chat(_)) {
                Some(message.sender().id())
            } else {
                None
            }
        } else if matches!(
            message.chat().type_(),
            ChatType::BasicGroup(_) | ChatType::Supergroup(_)
        ) {
            Some(message.sender().id())
        } else {
            None
        };

        if let Some(sender_id) = show_sender {
            let sender_name_expression = message.sender_display_name_expression();
            let sender_binding =
                sender_name_expression.bind(&*imp.message_bubble, "sender", glib::Object::NONE);
            bindings.push(sender_binding);

            imp.message_bubble.set_sender_id(sender_id);
        } else {
            imp.message_bubble.set_sender("");
            imp.message_bubble.set_sender_id(0);
        }

        let handler_id =
            message.connect_content_notify(clone!(@weak self as obj => move |message, _| {
                obj.update_document(message);
            }));
        imp.handler_id.replace(Some(handler_id));
        self.update_document(&message);

        // Setup caption expression
        let caption_binding = Message::this_expression("content")
            .chain_closure::<String>(closure!(|_: Message, content: BoxedMessageContent| {
                if let MessageContent::MessageDocument(data) = content.0 {
                    parse_formatted_text(data.caption)
                } else {
                    unreachable!();
                }
            }))
            .bind(&*imp.message_bubble, "label", Some(&message));

        imp.message.replace(Some(message));
        self.notify("message");
    }
}

#[derive(PartialEq)]
enum FileStatus {
    Downloading(f64),
    Uploading(f64),
    CanBeDownloaded,
    Downloaded,
}
use FileStatus::*;

impl From<&File> for FileStatus {
    fn from(file: &File) -> Self {
        let local = &file.local;
        let remote = &file.remote;

        let size = file.size.max(file.expected_size) as u64;

        if local.is_downloading_active {
            let progress = local.downloaded_size as f64 / size as f64;
            Downloading(progress)
        } else if remote.is_uploading_active {
            let progress = remote.uploaded_size as f64 / size as f64;
            Uploading(progress)
        } else if local.is_downloading_completed {
            Downloaded
        } else if local.can_be_downloaded {
            CanBeDownloaded
        } else {
            dbg!(file);
            unimplemented!("unknown file status");
        }
    }
}

impl MessageDocument {
    fn update_document(&self, message: &Message) {
        if let MessageContent::MessageDocument(data) = message.content().0 {
            let imp = self.imp();

            let message_text = parse_formatted_text(data.caption);

            imp.file_name_label.set_label(&data.document.file_name);

            let session = message.chat().session();

            self.update_status(data.document.document, session);
        }
    }

    fn update_status(&self, file: File, session: Session) -> FileStatus {
        let status = FileStatus::from(&file);

        let size = file.size.max(file.expected_size) as u64;

        self.update_size_label(&status, size);
        self.update_button(file, session, &status);

        status
    }

    fn update_button(&self, file: File, session: Session, status: &FileStatus) {
        let imp = self.imp();
        let click = &*imp.click;
        let image = &*imp.file_status_image;
        let file_id = file.id;

        let handler_id = match *status {
            Downloading(_progress) | Uploading(_progress) => {
                return;
                // Show loading indicator
            }
            CanBeDownloaded => {
                // Download file
                image.set_icon_name(Some("document-save-symbolic"));
                click.connect_released(clone!(@weak self as obj, @weak session => move |click, _, _, _| {
                    let (sender, receiver) = glib::MainContext::sync_channel::<File>(Default::default(), 5);
                    receiver.attach(
                        None,
                        clone!(@weak obj => @default-return glib::Continue(false), move |file| {
                            let message = obj.message();
                            if let MessageContent::MessageDocument(mut data) = message.content().0 {
                                data.document.document = file;
                                message.set_content(BoxedMessageContent(MessageContent::MessageDocument(data)));
                            }
                            glib::Continue(true)
                    }));

                    session.download_file(file_id, sender);

                    obj.imp().file_status_image.set_icon_name(Some("media-playback-stop-symbolic"));
                    let handler_id = click.connect_released(clone!(@weak session => move |_, _, _, _| {
                        session.cancel_file_download(file_id);
                    }));
                    if let Some(handler_id) = obj.imp().status_handler_id.replace(Some(handler_id)) {
                        click.disconnect(handler_id);
                    }
                }))
            }
            Downloaded => {
                // Open file
                image.set_icon_name(Some("folder-documents-symbolic"));
                let gio_file = gio::File::for_path(&file.local.path);
                click.connect_released(move |_, _, _, _| {
                    if let Err(err) = gio::AppInfo::launch_default_for_uri(
                        &gio_file.uri(),
                        gio::AppLaunchContext::NONE,
                    ) {
                        log::error!("Error: {}", err);
                    }
                })
            }
        };

        if let Some(handler_id) = imp.status_handler_id.replace(Some(handler_id)) {
            click.disconnect(handler_id);
        }
    }

    fn update_size_label(&self, status: &FileStatus, size: u64) {
        let size_label = &self.imp().file_size_label;

        match status {
            Downloading(progress) | Uploading(progress) => {
                let downloaded = glib::format_size((size as f64 * progress) as u64);
                let full_size = glib::format_size(size);

                size_label.set_label(&format!("{} / {}", downloaded, full_size));
            }
            CanBeDownloaded | Downloaded => {
                size_label.set_label(&glib::format_size(size));
            }
        }
    }
}
