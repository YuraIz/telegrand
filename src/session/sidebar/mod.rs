mod avatar;
mod mini_thumbnail;
mod row;
mod search;
mod selection;
mod session_switcher;

use self::row::Row;
use self::search::Search;
use self::selection::Selection;
use self::session_switcher::SessionSwitcher;

use glib::clone;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{glib, CompositeTemplate};

use crate::tdlib::Chat;
use crate::utils::spawn;
use crate::Session;

pub(crate) use self::avatar::Avatar;

mod imp {
    use super::*;
    use glib::subclass::Signal;
    use once_cell::sync::Lazy;
    use once_cell::unsync::OnceCell;
    use std::cell::{Cell, RefCell};

    use crate::session::components::{Avatar as ComponentsAvatar, Snow as ComponentsSnow};

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/melix99/telegrand/ui/sidebar.ui")]
    pub(crate) struct Sidebar {
        pub(super) compact: Cell<bool>,
        pub(super) selected_chat: RefCell<Option<Chat>>,
        pub(super) marked_as_unread_handler_id: RefCell<Option<glib::SignalHandlerId>>,
        pub(super) session: RefCell<Option<Session>>,
        pub(super) row_menu: OnceCell<gtk::PopoverMenu>,
        #[template_child]
        pub(super) snow: TemplateChild<ComponentsSnow>,
        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) main_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) session_switcher: TemplateChild<SessionSwitcher>,
        #[template_child]
        pub(super) selection: TemplateChild<Selection>,
        #[template_child]
        pub(super) search: TemplateChild<Search>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Sidebar {
        const NAME: &'static str = "Sidebar";
        type Type = super::Sidebar;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            ComponentsAvatar::static_type();
            Row::static_type();
            klass.bind_template();
            klass.bind_template_callbacks();

            klass.install_action("sidebar.start-search", None, move |widget, _, _| {
                widget.begin_chats_search();
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk::template_callbacks]
    impl Sidebar {
        #[template_callback]
        fn list_activate(&self, pos: u32) {
            self.selection.set_selected_position(pos);

            let chat = self.selection.selected_item().unwrap().downcast().unwrap();
            self.obj().select_chat(chat);
        }

        #[template_callback]
        fn close_search(&self) {
            self.stack.set_visible_child(&*self.main_box);
        }
    }

    impl ObjectImpl for Sidebar {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> =
                Lazy::new(|| vec![Signal::builder("chat-selected").build()]);
            SIGNALS.as_ref()
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecBoolean::builder("compact").build(),
                    glib::ParamSpecObject::builder::<Chat>("selected-chat")
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecObject::builder::<Session>("session")
                        .explicit_notify()
                        .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();

            match pspec.name() {
                "compact" => {
                    let compact = value.get().unwrap();
                    self.compact.set(compact);
                }
                "selected-chat" => {
                    let selected_chat = value.get().unwrap();
                    obj.set_selected_chat(selected_chat);
                }
                "session" => {
                    let session = value.get().unwrap();
                    obj.set_session(session);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();

            match pspec.name() {
                "compact" => self.compact.get().to_value(),
                "selected-chat" => obj.selected_chat().to_value(),
                "session" => obj.session().to_value(),
                _ => unimplemented!(),
            }
        }

        fn dispose(&self) {
            self.stack.unparent();
        }
    }

    impl WidgetImpl for Sidebar {}
}

glib::wrapper! {
    pub(crate) struct Sidebar(ObjectSubclass<imp::Sidebar>)
        @extends gtk::Widget;
}

impl Default for Sidebar {
    fn default() -> Self {
        Self::new()
    }
}

impl Sidebar {
    pub(crate) fn new() -> Self {
        glib::Object::builder().build()
    }

    pub(crate) fn row_menu(&self) -> &gtk::PopoverMenu {
        self.imp().row_menu.get_or_init(|| {
            gtk::Builder::from_resource("/com/github/melix99/telegrand/ui/sidebar-row-menu.ui")
                .object::<gtk::PopoverMenu>("menu")
                .unwrap()
        })
    }

    pub(crate) fn begin_chats_search(&self) {
        let imp = self.imp();
        imp.search.reset();
        imp.search.grab_focus();
        imp.stack.set_visible_child(&*imp.search);
    }

    pub(crate) fn select_chat(&self, chat: Chat) {
        self.set_selected_chat(Some(chat));
        self.emit_by_name::<()>("chat-selected", &[]);
    }

    pub(crate) fn selected_chat(&self) -> Option<Chat> {
        self.imp().selected_chat.borrow().clone()
    }

    pub(crate) fn set_selected_chat(&self, selected_chat: Option<Chat>) {
        if self.selected_chat() == selected_chat {
            return;
        }

        let imp = self.imp();

        if let Some(handler_id) = imp.marked_as_unread_handler_id.take() {
            self.selected_chat().unwrap().disconnect(handler_id);
        }

        if let Some(chat) = selected_chat.clone() {
            let handler_id = chat.connect_notify_local(
                Some("is-marked-as-unread"),
                clone!(@weak self as obj => move |chat, _| {
                    if chat.is_marked_as_unread() {
                        obj.set_selected_chat(None);
                    }
                }),
            );
            imp.marked_as_unread_handler_id.replace(Some(handler_id));

            if chat.is_marked_as_unread() {
                spawn(async move {
                    if let Err(e) = chat.mark_as_read().await {
                        log::warn!("Error on toggling chat's unread state: {e:?}");
                    }
                });
            }
        }

        imp.selection
            .set_selected_item(selected_chat.clone().map(Chat::upcast));

        imp.selected_chat.replace(selected_chat);
        self.notify("selected-chat");
    }

    pub(crate) fn set_session(&self, session: Option<Session>) {
        if self.session() == session {
            return;
        }

        let imp = self.imp();

        if let Some(ref session) = session {
            let filter = gtk::CustomFilter::new(|item| {
                let chat = item.downcast_ref::<Chat>().unwrap();
                chat.order() > 0
            });
            let sorter = gtk::CustomSorter::new(|item1, item2| {
                let chat1 = item1.downcast_ref::<Chat>().unwrap();
                let chat2 = item2.downcast_ref::<Chat>().unwrap();
                chat2.order().cmp(&chat1.order()).into()
            });

            session.chat_list().connect_positions_changed(
                clone!(@weak filter, @weak sorter => move |_| {
                    filter.changed(gtk::FilterChange::Different);
                    sorter.changed(gtk::SorterChange::Different);
                }),
            );

            let filter_model = gtk::FilterListModel::new(Some(session.chat_list()), Some(&filter));
            let sort_model = gtk::SortListModel::new(Some(&filter_model), Some(&sorter));

            imp.selection.set_model(Some(sort_model.upcast()));
        }

        imp.session.replace(session);
        self.notify("session");
    }

    pub(crate) fn session(&self) -> Option<Session> {
        self.imp().session.borrow().to_owned()
    }

    pub(crate) fn set_sessions(&self, sessions: &gtk::SelectionModel, this_session: &Session) {
        self.imp()
            .session_switcher
            .set_sessions(sessions, this_session);
    }

    pub(crate) fn connect_chat_selected<F: Fn(&Self) + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_local("chat-selected", true, move |values| {
            let obj = values[0].get::<Self>().unwrap();
            f(&obj);

            None
        })
    }
}
