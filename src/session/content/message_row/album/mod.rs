mod document;
mod photo;

use std::cell::RefCell;

use glib::closure;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use once_cell::sync::Lazy;
use tdlib::enums::MessageContent;

use super::base::MessageBaseExt;
use crate::session::content::message_row::MessageBase;
use crate::session::content::message_row::MessageBaseImpl;
use crate::session::content::message_row::MessageBubble;
use crate::tdlib::BoxedMessageContent;
use crate::tdlib::Message;
use crate::utils::parse_formatted_text;

use self::document::MessageAlbumDocument;
use self::photo::MessageAlbumPhoto;

mod imp {

    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(string = r#"
    using Adw 1;

    template $MessageAlbum : $MessageBase {
        layout-manager: BinLayout {};

        $MessageBubble message_bubble {
            styles ["media", "document"]

            prefix: Box {
                width-request: 300;

                orientation: vertical;

                $OriAnimatedGroup group {
                    styles ["album"]

                    spacing: 2;
                    overflow: hidden;

                    visible: false;
                }

                Box column {
                    orientation: vertical;
                    spacing: 6;

                    visible: false;
                }
            };
        }
    }
    "#)]
    pub(crate) struct MessageAlbum {
        pub(super) binding: RefCell<Option<gtk::ExpressionWatch>>,
        pub(super) album: RefCell<Option<glib::ValueArray>>,
        #[template_child]
        pub(super) message_bubble: TemplateChild<MessageBubble>,
        #[template_child]
        pub(super) group: TemplateChild<ori::AnimatedGroup>,
        #[template_child]
        pub(super) column: TemplateChild<gtk::Box>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageAlbum {
        const NAME: &'static str = "MessageAlbum";
        type Type = super::MessageAlbum;
        type ParentType = MessageBase;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessageAlbum {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecValueArray::builder("message")
                    .explicit_notify()
                    .build()]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();

            match pspec.name() {
                "message" => obj.set_message(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "message" => self.album.borrow().to_value(),
                _ => unimplemented!(),
            }
        }

        fn dispose(&self) {
            self.group.remove_chldren();
        }
    }

    impl WidgetImpl for MessageAlbum {}
    impl MessageBaseImpl for MessageAlbum {}
}

glib::wrapper! {
    pub(crate) struct MessageAlbum(ObjectSubclass<imp::MessageAlbum>)
        @extends gtk::Widget, MessageBase;
}

impl MessageBaseExt for MessageAlbum {
    type Message = glib::ValueArray;

    fn set_message(&self, album: Self::Message) {
        let imp = self.imp();

        let album_vec: Vec<Message> = album.into_iter().map(|v| v.get().unwrap()).collect();

        dbg!(&album_vec);

        if album_vec.len() == 0 {
            return;
        }

        // if let Some(binding) = imp.binding.take() {
        //     binding.unwatch();
        // }

        // if let Some(old_album) = imp.album.take() {
        //     let handler_id = imp.handler_id.take().unwrap();
        //     old_album.disconnect(handler_id);
        // }

        imp.album.replace(Some(album));

        let last_message = &album_vec.last().unwrap();

        imp.message_bubble.update_from_message(last_message, true);

        // Setup caption expression
        let caption_binding = Message::this_expression("content")
            .chain_closure::<String>(closure!(|_: Message, content: BoxedMessageContent| {
                let caption = match content.0 {
                    MessageContent::MessagePhoto(data) => data.caption,
                    MessageContent::MessageDocument(data) => data.caption,
                    _ => return format!("unimplemented caption: {:?}", content.0),
                };

                dbg!(parse_formatted_text(caption))
            }))
            .bind(&*imp.message_bubble, "label", Some(last_message.clone()));
        imp.binding.replace(Some(caption_binding));

        // TODO: reuse existing children
        imp.group.remove_chldren();
        while let Some(child) = imp.column.first_child() {
            child.unparent();
        }

        for message in album_vec.into_iter().rev() {
            match message.content().0 {
                MessageContent::MessagePhoto(_) => {
                    let photo = MessageAlbumPhoto::new();
                    photo.set_message(message);
                    imp.group.append(&photo);
                }
                MessageContent::MessageDocument(_) => {
                    let document = MessageAlbumDocument::new();
                    document.set_message(message);
                    imp.column.append(&document);
                }
                unsupported => {
                    log::debug!("unsupported message in album: {unsupported:?}");
                    continue;
                }
            }
        }

        imp.group.set_visible(imp.group.first_child().is_some());
        imp.column.set_visible(imp.column.first_child().is_some());

        self.notify("message");
    }
}

impl MessageAlbum {
    // fn update_photo(&self, message: &Message) {
    //     if let MessageContent::MessagePhoto(mut data) = message.content().0 {
    //         let imp = self.imp();
    //         // Choose the right photo size based on the screen scale factor.
    //         // See https://core.telegram.org/api/files#image-thumbnail-types for more
    //         // information about photo sizes.
    //         let photo_size = if self.scale_factor() > 2 {
    //             data.photo.sizes.pop().unwrap()
    //         } else {
    //             let type_ = if self.scale_factor() > 1 { "y" } else { "x" };

    //             match data.photo.sizes.iter().position(|s| s.r#type == type_) {
    //                 Some(pos) => data.photo.sizes.swap_remove(pos),
    //                 None => data.photo.sizes.pop().unwrap(),
    //             }
    //         };

    //         imp.picture
    //             .set_aspect_ratio(photo_size.width as f64 / photo_size.height as f64);

    //         if photo_size.photo.local.is_downloading_completed {
    //             self.load_photo(photo_size.photo.local.path);
    //         } else {
    //             imp.picture.set_paintable(
    //                 data.photo
    //                     .minithumbnail
    //                     .and_then(|m| {
    //                         gdk::Texture::from_bytes(&glib::Bytes::from_owned(glib::base64_decode(
    //                             &m.data,
    //                         )))
    //                         .ok()
    //                     })
    //                     .as_ref(),
    //             );

    //             let file_id = photo_size.photo.id;
    //             let session = message.chat().session();
    //             spawn(clone!(@weak self as obj, @weak session => async move {
    //                 obj.download_photo(file_id, &session).await;
    //             }));
    //         }
    //     }
    // }

    // async fn download_photo(&self, file_id: i32, session: &Session) {
    //     match session.download_file(file_id).await {
    //         Ok(file) => {
    //             self.load_photo(file.local.path);
    //         }
    //         Err(e) => {
    //             log::warn!("Failed to download a photo: {e:?}");
    //         }
    //     }
    // }

    // fn load_photo(&self, path: String) {
    //     let message_id = self.message().id();

    //     spawn(clone!(@weak self as obj => async move {
    //         let result = gio::spawn_blocking(move || decode_image_from_path(&path))
    //             .await
    //             .unwrap();

    //         // Check if the current message id is the same as the one at
    //         // the time of the request. It may be changed because of the
    //         // ListView recycling while decoding the image.
    //         if obj.message().id() != message_id {
    //             return;
    //         }

    //         match result {
    //             Ok(texture) => {
    //                 obj.imp().picture.set_paintable(Some(&texture));
    //             }
    //             Err(e) => {
    //                 log::warn!("Error decoding a photo: {e:?}");
    //             }
    //         }
    //     }));
    // }
}
