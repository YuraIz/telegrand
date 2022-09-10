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

use super::poll_option::PollOption;

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
    #[template(resource = "/com/github/melix99/telegrand/ui/content-message-poll.ui")]
    pub(crate) struct MessagePoll {
        pub(super) sender_color_class: RefCell<Option<String>>,
        pub(super) bindings: RefCell<Vec<gtk::ExpressionWatch>>,
        pub(super) status_handler_id: RefCell<Option<SignalHandlerId>>,
        pub(super) message: RefCell<Option<glib::Object>>,
        #[template_child]
        pub(super) sender_label: TemplateChild<gtk::Label>,
        // #[template_child]
        // pub(super) content_label: TemplateChild<MessageLabel>,
        #[template_child]
        pub(super) indicators: TemplateChild<MessageIndicators>,

        #[template_child]
        pub(super) question: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) caption: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) answers: TemplateChild<gtk::Box>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessagePoll {
        const NAME: &'static str = "ContentMessageDocument";
        type Type = super::MessagePoll;
        type ParentType = MessageBase;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.set_css_name("messagepoll");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessagePoll {
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

    impl WidgetImpl for MessagePoll {}
    impl MessageBaseImpl for MessagePoll {}
}

glib::wrapper! {
    pub(crate) struct MessagePoll(ObjectSubclass<imp::MessagePoll>)
        @extends gtk::Widget, MessageBase;
}

impl MessageBaseExt for MessagePoll {
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

            self.update_poll(message);
        } else {
            unreachable!("Unexpected message type: {:?}", message);
        }

        imp.message.replace(Some(message));
        self.notify("message");
    }
}

impl MessagePoll {
    fn update_poll(&self, message: &Message) {
        if let MessageContent::MessagePoll(data) = message.content().0 {
            let imp = self.imp();

            imp.question.set_label(&data.poll.question);

            let is_anonymous = data.poll.is_anonymous;

            if is_anonymous {
                imp.caption.set_label("anonymous poll");
            } else {
                imp.caption.set_label("public poll");
            }

            dbg!(&data);

            for option in &data.poll.options {
                imp.answers.append(&PollOption::from(option));
            }

            match (data.poll.r#type, data.poll.is_anonymous) {
                (PollType::Regular(_), true) => {
                    imp.caption.set_label("anonymous poll");
                }
                (PollType::Regular(_), false) => {
                    imp.caption.set_label("public poll");
                }

                (PollType::Quiz(_), true) => {
                    imp.caption.set_label("anonymous quiz");
                }

                (PollType::Quiz(_), false) => {
                    imp.caption.set_label("public quiz");
                }
            }
        }
    }
}
