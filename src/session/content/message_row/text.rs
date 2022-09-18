use gettextrs::gettext;
use glib::closure;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{glib, CompositeTemplate};
use tdlib::enums::MessageContent;

use crate::session::content::message_row::{
    MessageBase, MessageBaseImpl, MessageBubble, MessageIndicators,
};
use crate::tdlib::{BoxedMessageContent, Chat, ChatType, Message, MessageSender, SponsoredMessage};
use crate::utils::parse_formatted_text;

use super::base::MessageBaseExt;

mod imp {
    use super::*;
    use once_cell::sync::Lazy;
    use std::cell::RefCell;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/melix99/telegrand/ui/content-message-text.ui")]
    pub(crate) struct MessageText {
        pub(super) bindings: RefCell<Vec<gtk::ExpressionWatch>>,
        pub(super) message: RefCell<Option<glib::Object>>,
        #[template_child]
        pub(super) message_bubble: TemplateChild<MessageBubble>,
        #[template_child]
        pub(super) indicators: TemplateChild<MessageIndicators>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageText {
        const NAME: &'static str = "ContentMessageText";
        type Type = super::MessageText;
        type ParentType = MessageBase;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.set_layout_manager_type::<gtk::BinLayout>();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessageText {
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

    impl WidgetImpl for MessageText {}
    impl MessageBaseImpl for MessageText {}
}

glib::wrapper! {
    pub(crate) struct MessageText(ObjectSubclass<imp::MessageText>)
        @extends gtk::Widget, MessageBase;
}

impl MessageBaseExt for MessageText {
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

        if let Some(message) = message.downcast_ref::<Message>() {
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

            // Set content label expression
            let text_binding = Message::this_expression("content")
                .chain_closure::<String>(closure!(|_: Message, content: BoxedMessageContent| {
                    format_message_content_text(content.0)
                }))
                .bind(&*imp.message_bubble, "label", Some(message));
            bindings.push(text_binding);
        } else if let Some(sponsored_message) = message.downcast_ref::<SponsoredMessage>() {
            let sender_binding = Chat::this_expression("title").bind(
                &*imp.message_bubble,
                "sender",
                Some(&sponsored_message.sponsor_chat()),
            );
            bindings.push(sender_binding);

            let text_binding = SponsoredMessage::this_expression("content")
                .chain_closure::<String>(closure!(
                    |_: SponsoredMessage, content: BoxedMessageContent| {
                        format_message_content_text(content.0)
                    }
                ))
                .bind(&*imp.message_bubble, "label", Some(sponsored_message));
            bindings.push(text_binding);
        } else {
            unreachable!("Unexpected message type: {:?}", message);
        }

        imp.message.replace(Some(message));
        self.notify("message");
    }
}

fn format_message_content_text(content: MessageContent) -> String {
    match content {
        MessageContent::MessageText(content) => parse_formatted_text(content.text),
        _ => format!("<i>{}</i>", gettext("This message is unsupported")),
    }
}
