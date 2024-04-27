use super::item::ItemPage;
use super::movie::MoviePage;
use super::tu_list_item::TuListItem;
use super::window::Window;
use crate::client::{network::*, structs::*};
use crate::ui::provider::tu_item::TuItem;
use crate::utils::{get_data_with_cache, spawn, spawn_tokio};
use adw::prelude::NavigationPageExt;
use glib::Object;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};
mod imp {

    use std::cell::{OnceCell, RefCell};

    use glib::subclass::InitializingObject;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::{glib, CompositeTemplate};

    use crate::utils::spawn_g_timeout;

    // Object holding the state
    #[derive(CompositeTemplate, Default, glib::Properties)]
    #[template(resource = "/moe/tsukimi/list.ui")]
    #[properties(wrapper_type = super::ListPage)]
    pub struct ListPage {
        #[property(get, set, construct_only)]
        pub id: OnceCell<String>,
        #[property(get, set, construct_only)]
        pub collectiontype: OnceCell<String>,
        #[template_child]
        pub listgrid: TemplateChild<gtk::GridView>,
        #[template_child]
        pub spinner: TemplateChild<gtk::Spinner>,
        #[template_child]
        pub listrevealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub count: TemplateChild<gtk::Label>,
        #[template_child]
        pub listscrolled: TemplateChild<gtk::ScrolledWindow>,
        pub selection: gtk::SingleSelection,
        pub popovermenu: RefCell<Option<gtk::PopoverMenu>>,
    }

    // The central trait for subclassing a GObject
    #[glib::object_subclass]
    impl ObjectSubclass for ListPage {
        // `NAME` needs to match `class` attribute of template
        const NAME: &'static str = "ListPage";
        type Type = super::ListPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    // Trait shared by all GObjects
    #[glib::derived_properties]
    impl ObjectImpl for ListPage {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            spawn_g_timeout(glib::clone!(@weak obj => async move {
                obj.set_factory().await;
            }));
        }
    }

    // Trait shared by all widgets
    impl WidgetImpl for ListPage {}

    // Trait shared by all windows
    impl WindowImpl for ListPage {}

    // Trait shared by all application windows
    impl ApplicationWindowImpl for ListPage {}

    impl adw::subclass::navigation_page::NavigationPageImpl for ListPage {}
}

glib::wrapper! {
    pub struct ListPage(ObjectSubclass<imp::ListPage>)
        @extends gtk::ApplicationWindow, gtk::Window, gtk::Widget ,adw::NavigationPage,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl ListPage {
    pub fn new(id: String, collection_type: String) -> Self {
        Object::builder()
            .property("id", id)
            .property("collectiontype", collection_type)
            .build()
    }

    async fn set_factory(&self) {
        let imp = self.imp();
        let spinner = imp.spinner.get();
        let listrevealer = imp.listrevealer.get();
        let count = imp.count.get();
        let id = imp.id.get().expect("id not set").clone();
        let c = imp.collectiontype.get().unwrap();
        let include_item_types = match c.as_str() {
            "movies" => "Movie",
            "tvshows" => "Series",
            "music" => "MusicAlbum",
            _ => "Movie, Series",
        };
        spinner.set_visible(true);
        let list_results = get_data_with_cache(id.to_string(), &include_item_types, async move {
            get_list(id.to_string(), 0.to_string(), &include_item_types).await
        })
        .await
        .unwrap();
        let store = gio::ListStore::new::<glib::BoxedAnyObject>();
        spawn(glib::clone!(@weak store=> async move {
                for result in list_results.items {
                    let object = glib::BoxedAnyObject::new(result);
                    store.append(&object);
                }
                spinner.set_visible(false);
                count.set_text(&format!("{} Items",list_results.total_record_count));
                listrevealer.set_reveal_child(true);
        }));
        imp.selection.set_model(Some(&store));
        let factory = gtk::SignalListItemFactory::new();
        /* factory.connect_setup(move |_, item| {
            let list_item = item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem");
            let listbox = gtk::Box::new(gtk::Orientation::Vertical, 5);
            let picture = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .height_request(260)
                .width_request(167)
                .valign(gtk::Align::Start)
                .homogeneous(true)
                .build();
            let label = gtk::Label::builder()
                .valign(gtk::Align::Start)
                .halign(gtk::Align::Center)
                .justify(gtk::Justification::Center)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .build();
            listbox.append(&picture);
            listbox.append(&label);
            listbox.set_valign(gtk::Align::Start);
            list_item.set_child(Some(&listbox));
        }); */
        factory.connect_bind(move |_, item| {
            let list_item = item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem");
            let entry = item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem")
                .item()
                .and_downcast::<glib::BoxedAnyObject>()
                .expect("Needs to be BoxedAnyObject");
            let latest: std::cell::Ref<Latest> = entry.borrow();
            if list_item.child().is_none() {
                match latest.latest_type.as_str() {
                    "Movie" => {
                        let tu_item: TuItem = glib::object::Object::new();
                        tu_item.set_id(latest.id.clone());
                        tu_item.set_name(latest.name.clone());
                        tu_item.set_production_year(latest.production_year.unwrap());
                        if let Some(userdata) = &latest.user_data {
                            tu_item.set_played(userdata.played);
                        }
                        let list_child = TuListItem::new(tu_item, "Movie");
                        list_item.set_child(Some(&list_child));
                    }
                    "Series" => {
                        let tu_item: TuItem = glib::object::Object::new();
                        tu_item.set_id(latest.id.clone());
                        tu_item.set_name(latest.name.clone());
                        tu_item.set_production_year(latest.production_year.unwrap());
                        if let Some(userdata) = &latest.user_data {
                            tu_item.set_played(userdata.played);
                            tu_item.set_unplayed_item_count(userdata.unplayed_item_count.unwrap());
                        }
                        let list_child = TuListItem::new(tu_item, "Series");
                        list_item.set_child(Some(&list_child));
                    }
                    _ => {}
                }
            }
            /* let picture = item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem")
                .child()
                .and_downcast::<gtk::Box>()
                .expect("Needs to be Box")
                .first_child()
                .expect("Needs to be Picture");
            let label = item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem")
                .child()
                .and_downcast::<gtk::Box>()
                .expect("Needs to be Box")
                .last_child()
                .expect("Needs to be Picture");
            let entry = item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem")
                .item()
                .and_downcast::<glib::BoxedAnyObject>()
                .expect("Needs to be BoxedAnyObject");
            let latest: std::cell::Ref<Latest> = entry.borrow();
            if latest.latest_type == "MusicAlbum" {
                picture.set_size_request(167, 167);
            } else {
                picture.set_size_request(167, 277);
            }
            if picture.is::<gtk::Box>() {
                if let Some(_revealer) = picture
                    .downcast_ref::<gtk::Box>()
                    .expect("Needs to be Box")
                    .first_child()
                {
                } else {
                    let img = crate::ui::image::setimage(latest.id.clone());
                    let overlay = gtk::Overlay::builder().child(&img).build();
                    if let Some(userdata) = &latest.user_data {
                        if let Some(unplayeditemcount) = userdata.unplayed_item_count {
                            if unplayeditemcount > 0 {
                                let mark = gtk::Label::new(Some(
                                    &userdata
                                        .unplayed_item_count
                                        .expect("no unplayeditemcount")
                                        .to_string(),
                                ));
                                mark.set_valign(gtk::Align::Start);
                                mark.set_halign(gtk::Align::End);
                                mark.set_height_request(40);
                                mark.set_width_request(40);
                                overlay.add_overlay(&mark);
                            }
                        }
                    }
                    picture
                        .downcast_ref::<gtk::Box>()
                        .expect("Needs to be Box")
                        .append(&overlay);
                }
            }
            if label.is::<gtk::Label>() {
                let mut str = latest.name.to_string();
                if let Some(productionyear) = latest.production_year {
                    str.push_str(&format!("\n{}", productionyear));
                }
                label
                    .downcast_ref::<gtk::Label>()
                    .expect("Needs to be Label")
                    .set_text(&str);
            } */
        });
        imp.listgrid.set_factory(Some(&factory));
        imp.listgrid.set_model(Some(&imp.selection));
        imp.listgrid.set_min_columns(3);
        imp.listgrid.set_max_columns(13);
        imp.listgrid.connect_activate(
            glib::clone!(@weak self as obj => move |gridview, position| {
                let model = gridview.model().unwrap();
                let item = model.item(position).and_downcast::<glib::BoxedAnyObject>().unwrap();
                let result: std::cell::Ref<Latest> = item.borrow();
                let window = obj.root().and_downcast::<Window>().unwrap();
                if result.latest_type == "Movie" {
                    window.set_title(&result.name);
                    let item_page = MoviePage::new(result.id.clone(),result.name.clone());
                    item_page.set_tag(Some(&result.name));
                    window.imp().homeview.push(&item_page);
                } else if result.latest_type == "Series" {
                    window.set_title(&result.name);
                    let item_page = ItemPage::new(result.id.clone(),result.id.clone());
                    item_page.set_tag(Some(&result.name));
                    window.imp().homeview.push(&item_page);
                }
                std::env::set_var("HOME_TITLE", &result.name);
            }),
        );
        self.update().await;
    }

    pub async fn update(&self) {
        let scrolled = self.imp().listscrolled.get();
        scrolled.connect_edge_overshot(glib::clone!(@weak self as obj => move |_, pos| {
            if pos == gtk::PositionType::Bottom {
                let spinner = obj.imp().spinner.get();
                spinner.set_visible(true);
                let store = obj.imp().selection.model().unwrap().downcast::<gio::ListStore>().unwrap();
                let id = obj.imp().id.get().expect("id not set").clone();
                let offset = obj.imp().selection.model().unwrap().n_items();
                let c = obj.imp().collectiontype.get().unwrap();
                let include_item_types = match c.as_str() {
                    "movies" => {
                        "Movie"
                    }
                    "tvshows" => {
                        "Series"
                    }
                    "music" => {
                        "MusicAlbum"
                    }
                    _ => {
                        "Movie, Series"
                    }
                };
                spinner.set_visible(true);
                let list_results = spawn_tokio(async move {
                    get_list(id.to_string(),offset.to_string(),&include_item_types).await.unwrap()
                });
                spawn(glib::clone!(@weak store=> async move {
                        let list_results = list_results.await;
                        for result in list_results.items {
                            let object = glib::BoxedAnyObject::new(result);
                            store.append(&object);
                        }
                        spinner.set_visible(false);
                }));
            }
        }));
    }
}
