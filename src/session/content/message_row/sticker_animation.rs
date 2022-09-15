use glib::clone;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib, CompositeTemplate};
use image::io::Reader as ImageReader;
use image::ImageFormat;
use std::io::Cursor;
use tdlib::enums::MessageContent;
use tdlib::types::File;

use crate::session::content::message_row::{
    MessageBase, MessageBaseImpl, MessageIndicators, StickerPicture,
};
use crate::tdlib::Message;
use crate::utils::spawn;

use super::base::MessageBaseExt;
use super::lottie_animation::LottieAnimation;

mod imp {
    use super::*;
    use once_cell::sync::Lazy;
    use std::cell::RefCell;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/melix99/telegrand/ui/content-message-sticker-animation.ui")]
    pub(crate) struct MessageStickerAnimation {
        pub(super) file_path: RefCell<String>,
        pub(super) message: RefCell<Option<Message>>,
        #[template_child]
        pub(super) image: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) indicators: TemplateChild<MessageIndicators>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageStickerAnimation {
        const NAME: &'static str = "ContentMessageStickerAnimation";
        type Type = super::MessageStickerAnimation;
        type ParentType = MessageBase;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.set_css_name("messagesticker");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessageStickerAnimation {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecObject::new(
                    "message",
                    "Message",
                    "The message represented by this row",
                    Message::static_type(),
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

    impl WidgetImpl for MessageStickerAnimation {
        fn map(&self, widget: &Self::Type) {
            self.parent_map(widget);
            widget.load_sticker();
        }
        fn unmap(&self, widget: &Self::Type) {
            self.parent_unmap(widget);
            self.image.set_paintable(gdk::Paintable::NONE);
        }
    }
    impl MessageBaseImpl for MessageStickerAnimation {}
}

glib::wrapper! {
    pub(crate) struct MessageStickerAnimation(ObjectSubclass<imp::MessageStickerAnimation>)
        @extends gtk::Widget, MessageBase;
}

impl MessageBaseExt for MessageStickerAnimation {
    type Message = Message;

    fn set_message(&self, message: Self::Message) {
        let imp = self.imp();

        if imp.message.borrow().as_ref() == Some(&message) {
            return;
        }

        imp.indicators.set_message(message.clone().upcast());

        if let MessageContent::MessageSticker(data) = message.content().0 {
            if data.sticker.sticker.local.is_downloading_completed {
                self.imp()
                    .file_path
                    .replace(data.sticker.sticker.local.path.to_string());
                self.load_sticker();
            } else {
                let (sender, receiver) =
                    glib::MainContext::sync_channel::<File>(Default::default(), 5);

                receiver.attach(
                    None,
                    clone!(@weak self as obj => @default-return glib::Continue(false), move |file| {
                        if file.local.is_downloading_completed {
                            obj.imp().file_path.replace(file.local.path.to_string());
                            obj.load_sticker();
                        }

                        glib::Continue(true)
                    }),
                );

                message
                    .chat()
                    .session()
                    .download_file(data.sticker.sticker.id, sender);
            }
        }
        imp.message.replace(Some(message));

        self.notify("message");
    }
}

impl MessageStickerAnimation {
    fn load_sticker(&self) {
        let path = &*self.imp().file_path.borrow();
        if path.is_empty() {
            return;
        }

        let image = &*self.imp().image;

        let media_file = if path.ends_with("webm") {
            gtk::MediaFile::for_filename(path)
        } else {
            LottieAnimation::from_filename(path).upcast()
        };

        media_file.set_loop(true);

        media_file.play();

        image.set_paintable(Some(&media_file));
    }
}
