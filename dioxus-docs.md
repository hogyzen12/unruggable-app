Your First Component
Now that we've initialized our HotDog app, we can start building out its components.

What is a component?
In Dioxus, apps are comprised of individual functions called Components that take in some Properties and render an Element:

src/guide_component.rs

fn DogApp(props: DogAppProps) -> Element {
    todo!()
}
Component Properties
All components take an object that outlines which parameters the component can accept. All Props structs in Dioxus need to derive the Properties trait which requires both Clone and PartialEq:

src/guide_component.rs

#[derive(Props, PartialEq, Clone)]
struct DogAppProps {
    breed: String,
}
Dioxus provides the #[component] macro for simplifying how components are defined. This macro converts the parameters of the annotated function into a hidden accompanying struct.

src/guide_component.rs

#[component]
fn DogApp(breed: String) -> Element {
    todo!()
}
When building apps, you'll frequently use the #[component] macro. When building libraries, we generally suggest deriving Props instead.

Properties are Immutable
If you're familiar with JavaScript, then you might also be familiar with libraries like React. Dioxus is very similar to React: if you know React then you will feel comfortable with Dioxus.

Just like React, Dioxus components are rendered by calling the function component. On every render, Dioxus makes a .clone() of the component's props. This ensures you can't accidentally modify your props which can lead to hard-to-track issues with state management.

src/guide_component.rs

#[component]
fn DogApp(breed: String) -> Element {
    tracing::info!("Rendered with breed: {breed}");

    todo!()
}
Dioxus provides types that make .clone() cheaper to call, so don't worry about performance gotchas here.

Component Functions are Called Multiple Times
Just like React, Dioxus will call your component function multiple times throughout its lifecycle. This is called re-rendering. In Dioxus, re-renders are extremely cheap (much cheaper than React!). In most cases you shouldn't worry about re-rendering too frequently.

When Dioxus re-renders your component, it compares the Element returned from the last render against the Element returned in the current render.

For example, when the breed property changes on the DogApp component, Dioxus will call the DogApp function a second time and compare the previous Element against the new Element.

src/guide_component.rs

#[component]
fn DogApp(breed: String) -> Element {
    rsx! {
        "Breed: {breed}"
    }
}
Diffing

Dioxus will re-render your component in only two circumstances:

When the Props change as determined by PartialEq
When a function like signal.set() or signal.write() calls Scope.needs_update()
Unlike React, all Dioxus components are memoized by default meaning Dioxus will always compare Props before deciding to re-render your component. As an additional optimization, Dioxus only compares dynamic parts of your RSX. Elements that don't contain dynamic data won't be checked for changes.

Composing Components
In Dioxus, Components are composed together to create Apps. Each component will hold onto its own state and handle its own updates. This makes it easy to abstract your app into different parts and even share pieces of your app as libraries for others to use.

To compose components together, we'll use the rsx! {} macro to define the structure of our app.

src/guide_component.rs

#[component]
fn App() -> Element {
    rsx! {
        Header {}
        DogApp { breed: "corgi" }
        Footer {}
    }
}
We'll cover rsx! {} in more depth in the next chapter.

Describing the UI
Dioxus is a declarative framework. This means that instead of telling Dioxus what to do (e.g. to "create an element" or "set the color to red") we simply declare what we want the UI to look like using RSX.

src/hello_world_desktop.rs

// define a component that renders a div with the text "Hello, world!"
fn App() -> Element {
    rsx! {
        div { "Hello, world!" }
    }
}
Here, we use the rsx! macro to declare that we want a div element, containing the text "Hello, world!". Dioxus takes the RSX and constructs a user interface from it.

Editing RSX with Hot-Reloading
When using dx serve, your app's RSX is automatically hot-reloaded whenever you edit and save the file. You can edit RSX structure, add new elements, and style your markup without a full rebuild.

Whenever you edit Rust code, then dx will automatically force a "full rebuild" of your app.


For an in-depth guide in what can and can't be hot-reloaded, check the hot-reload guide in the reference.

RSX is just HTML
Dioxus provides the rsx! {} macro for assembling Elements in your app. The rsx! {} macro primarily speaks HTML: the web, desktop, and mobile Dioxus first-party renderers all use HTML and CSS as the layout and styling technologies.

This means you can reuse your knowledge of the web and build your app using div, span, img, style, button, and more.

The RSX syntax is a "strict" form of Rust that uses Rust's Struct syntax for assembling elements:

src/guide_rsx.rs

rsx! {
    div {
        class: "bg-red-100"
    }
}
Elements in RSX differ slightly from Rust struct syntax: they can also contain child structs placed immediately after the final attribute.

src/guide_rsx.rs

rsx! {
    div { class: "bg-red-100",
        button {
            onclick: move |_| info!("Clicked"),
            "Click me!"
        }
    }
}
Additionally, all quoted strings in RSX imply format!() automatically, so you can define a variable outside your markup and use it in your strings without an explicit format call:

src/guide_rsx.rs

rsx! {
    div { "Breed: {breed}" }
}
Any expression that can be rendered to a String can be included directly in RSX. RSX also accepts Option<Element> and iterators of Elements:

src/guide_rsx.rs

rsx! {
    // Anything that's `Display`
    {"Something"}

    // Optionals
    {show_title.then(|| rsx! { "title!" } )}

    // And iterators
    ul {
        {(0..5).map(|i| rsx! { "{i}" })}
    }
}
Dioxus provides two items of syntax sugar for these common cases: for loops and if chains. These blocks return the contained RSX directly.

src/guide_rsx.rs

rsx! {
    if show_title {
        "title!"
    }

    ul {
        for item in 0..5 {
            "{item}"
        }
    }
}
For lists, Dioxus uses the key attribute to ensure it's comparing the right elements between renders. If you forget to add a key attribute to your list item, you might run into performance and state management issues. Usually you can find a unique key to differentiate your list items:

src/guide_rsx.rs

rsx! {
    for user in users {
        div {
            key: "{user.id}",
            "{user.name}"
        }
    }
}
Adding UI to our HotDog App
Let's add a basic UI to our app. We'll add a header, a body image for the dog photo, and some basic buttons.

src/guide_rsx.rs

#[component]
fn App() -> Element {
    rsx! {
        div { id: "title",
            h1 { "HotDog! 🌭" }
        }
        div { id: "dogview",
            img { src: "https://images.dog.ceo/breeds/pitbull/dog-3981540_1280.jpg" }
        }
        div { id: "buttons",
            button { id: "skip", "skip" }
            button { id: "save", "save!" }
        }
    }
}
Our app is coming together!

Unstyled Dog App

Unfortunately, it's not very beautiful yet. Let's move on to styling our app.

Styling and Assets
Unfortunately, our HotDog app isn't quite ready to show off - it's completely unstyled!

In this chapter we'll cover adding assets and styles to our app.

Dioxus uses CSS for Styling
As mentioned earlier, Dioxus apps use HTML and CSS as the core markup and styling technology. Instead of re-inventing the wheel like Flutter and React-Native, we designed Dioxus to use HTML and CSS on every platform.

CSS is by-far the most popular styling system and is extremely capable. For example, here's a screenshot of ebou, a very beautiful Mastodon client built with Dioxus.

Ebou

HTML and CSS are very powerful - don't worry about being too limited!

Adding the CSS File with asset!()
The bare-bones template already includes a base main.css in the assets folder.


├── Cargo.toml
├── assets
│   └── main.css
└── src
    └── main.rs
To include the CSS in our app, we can use the asset!() macro. This macro ensures the asset will be included in the final app bundle.

src/guide_assets.rs

static CSS: Asset = asset!("/assets/main.css");
We also need to load the asset into our app using the document::Stylesheet component. This component is equivalent to the <link> HTML element but also ensures the CSS will be pre-loaded during server-side-rendering.

src/guide_assets.rs

fn App() -> Element {
    rsx! {
        document::Stylesheet { href: CSS }
    }
}
Unlike Rust's include_str!() macro, the asset!() macro does not actually include the contents of the asset in our final executable. Instead, it generates a unique path so that the asset can be loaded at runtime. This is ideal for web apps where assets are loaded in parallel through different HTTP requests.

📣 The asset!() macro generates a unique name that won't exactly match the input name. This helps prevents name collisions and improves caching.

Hot-Reloading
All assets in Dioxus participate in hot-reloading. Try editing your app's main.css and watch changes propagate in real time.


Including Images
In Dioxus, you can include images in two ways:

Dynamically with a URL
Statically with the asset!() macro.
When including assets with a URL, simply fill the src attribute of img {}. Note that when the app is offline, URL-based images won't download.

src/guide_assets.rs

rsx! {
    // ...
    div {
        img { src: "https://images.dog.ceo/breeds/pitbull/dog-3981540_1280.jpg" }
    }
}
For static images, you can use the same asset!() macro that we used to include the app's CSS.

src/guide_assets.rs

static ICON: Asset = asset!("/assets/icon.png");

rsx! {
    img { src: ICON }
}
Optimizations
By default, the asset!() macro will lightly optimize CSS, JavaScript, JSON, and images. The name of the asset will also be modified to include a content hash.

src/guide_assets.rs

// would output main-j1238nask123.css
asset!("/assets/main.css").to_string();
You can optimize assets even further, with an optional Options struct. For example, dx can automatically convert .png images to a more optimized .avif format:

src/guide_assets.rs

// outputs icon-j1238jd2.avif
asset!("/assets/icon.png", ImageAssetOptions::new().with_avif());
For many apps, asset optimization is the most effective way of improving load times. As developers, we frequently overlook the size of images and accidentally make our sites load slower.

Check out the assets guide for a more in-depth explanation of how the Dioxus asset system works.

The Final CSS
We can use the asset hot-reload system of dx and our knowledge of CSS to create a beautiful app:

Styled Dog App

The final CSS is here for reference:


/* App-wide styling */
html, body {
    background-color: #0e0e0e;
    color: white;
    font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
    height: 100%;
    width: 100%;
    overflow: hidden;
    margin: 0;
}

#main {
    display: flex;
    flex-direction: column;
    height: 100%;
    justify-content: space-between;
}

#dogview {
    max-height: 80vh;
    flex-grow: 1;
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
}

#dogview img {
    display: block;
    max-width: 50%;
    max-height: 50%;
    transform: scale(1.8);
    border-radius: 5px;
    border: 1px solid rgb(233, 233, 233);
    box-shadow: 0px 0px 5px 1px rgb(216, 216, 216, 0.5);
}

#title {
    text-align: center;
    padding-top: 10px;
    border-bottom: 1px solid #a8a8a8;
    display: flex;
    flex-direction: row;
    justify-content: space-evenly;
    align-items: center;
}

#title a {
    text-decoration: none;
    color: white;
}

a#heart {
    background-color: white;
    color: red;
    padding: 5px;
    border-radius: 5px;
}

#title span {
    width: 20px;
}

#title h1 {
    margin: 0.25em;
    font-style: italic;
}

#buttons {
    display: flex;
    flex-direction: row;
    justify-content: center;
    gap: 20px;
    /* padding-top: 20px; */
    padding-bottom: 20px;
}

#skip { background-color: gray }
#save { background-color: green; }

#skip, #save {
    padding: 5px 30px 5px 30px;
    border-radius: 3px;
    font-size: 2rem;
    font-weight: bold;
    color: rgb(230, 230, 230)
}

#navbar {
    border: 1px solid rgb(233, 233, 233);
    border-width: 1px 0px 0px 0px;
    display: flex;
    flex-direction: row;
    justify-content: space-evenly;
    padding: 20px;
    gap: 20px;
}

#navbar a {
    background-color: #a8a8a8;
    border-radius: 5px;
    border: 1px solid black;
    text-decoration: none;
    color: black;
    padding: 10px 30px 10px 30px;
}

#favorites {
    flex-grow: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    padding: 10px;
}

#favorites-container {
    overflow-y: auto;
    overflow-x: hidden;
    display: flex;
    flex-direction: row;
    flex-wrap: wrap;
    justify-content: center;
    gap: 10px;
    padding: 10px;
}

.favorite-dog {
    max-height: 180px;
    max-width: 60%;
    position: relative;
}

.favorite-dog img {
    max-height: 150px;
    border-radius: 5px;
    margin: 5px;
}

.favorite-dog:hover button {
    display: block;
}

.favorite-dog button {
    display: none;
    position: absolute;
    bottom: 10px;
    left: 10px;
    z-index: 10;
}



Interactivity
Now that our HotDog app is scaffolded and styled, we can finally add some interactive elements.

Encapsulating State
Before we get too far, let's split our app into two parts: the Title and the DogView. This will help us organize our app and keep the DogView state separated from Title state.

src/guide_state.rs

#[component]
fn App() -> Element {
    rsx! {
        document::Stylesheet { href: CSS }
        Title {}
        DogView {}
    }
}

#[component]
fn Title() -> Element {
    rsx! {
        div { id: "title",
            h1 { "HotDog! 🌭" }
        }
    }
}

#[component]
fn DogView() -> Element {
    rsx! {
        div { id: "dogview",
            img { src: "https://images.dog.ceo/breeds/pitbull/dog-3981540_1280.jpg" }
        }
        div { id: "buttons",
            button { id: "skip", "skip" }
            button { id: "save", "save!" }
        }
    }
}
Event Handlers
In the DogView component, we want to attach an action to the click of the buttons. For example: skipping or saving the current dog photo. We can use an EventHandler to listen for the click events.

Event handlers are similar to regular attributes, but their name usually starts with on - and they accept closures as values. The closure will be called whenever its corresponding event is triggered. The listener receives information about the event in the Event object.

We'll add some closures inline and then pass them to the onclick attribute for both the skip and save buttons:

src/guide_state.rs

#[component]
fn DogView() -> Element {
    let skip = move |evt| {};
    let save = move |evt| {};

    rsx! {
        // ...
        div { id: "buttons",
            button { onclick: skip, id: "skip",  "skip" }
            button { onclick: save, id: "save",  "save!" }
        }
    }
}
You can read more about Event Handlers in the Event Handler reference

State with use_hook
So far, our components have no internal state. For our DogView, we want to change the currently displayed dog photo whenever the user clicks skip or save.

To store state in components, Dioxus provides the use_hook function. This makes it possible for bare Rust functions to store and load state without the use of an extra struct.

When called in a component, the use_hook function will return a .clone() of the originally stored value:

src/guide_state.rs

#[component]
fn DogView() -> Element {
    let img_src = use_hook(|| "https://images.dog.ceo/breeds/pitbull/dog-3981540_1280.jpg");

    // ..

    rsx! {
        div { id: "dogview",
            img { src: "{img_src}" }
        }
        // ..
    }
}
Dioxus hooks are very similar to React's hooks and need to follow some simple rules to function properly.

Signals and use_signal
While use_hook makes it possible to store any value that implements Clone, you'll frequently want a more capable form of state management. Built-in to Dioxus are signals.

Signal is a wrapper type around an ordinary Rust value that tracks reads and writes, bringing your app to life. You can wrap any Rust value in a signal. Signals can be created manually with Signal::new() but we strongly recommend using the use_signal hook instead.

📣 Manually creating Signals requires remembering to call .manually_drop() on the signal whereas use_signal cleans the Signal up for you automatically.

Whenever a signal's value changes, its containing "reactive scope" will be "marked dirty" and re-run. By default, Dioxus components are reactive scopes, and thus, will re-render whenever a signal value changes.


Signals are core to Dioxus and take time to master. We recommend reading the state management guide in depth before diving into your first large app.

Global State with Context
While hooks are good for state local to components, occasionally you'll want to manage state for your entire app.

Dioxus provides two mechanisms: Context and GlobalSignal.

The Context API makes it possible for parent components to share state with child components without explicitly declaring an additional property field. This is used by larger apps and libraries to share state across the app without modifying component signatures.

To "provide" context, simply call use_context_provider() with a struct that implements Clone. To read the context in a child, call use_context().

src/guide_state.rs

// Create a new wrapper type
#[derive(Clone)]
struct TitleState(String);

fn App() -> Element {
    // Provide that type as a Context
    use_context_provider(|| TitleState("HotDog".to_string()));
    rsx! {
        Title {}
    }
}

fn Title() -> Element {
    // Consume that type as a Context
    let title = use_context::<TitleState>();
    rsx! {
        h1 { "{title.0}" }
    }
}
You can combine use_signal and Context to provide reactive state to your app:

src/guide_state.rs

#[derive(Clone, Copy)]
struct MusicPlayer {
    song: Signal<String>,
}

fn use_music_player_provider() {
    let song = use_signal(|| "Drift Away".to_string());
    use_context_provider(|| MusicPlayer { song });
}
With use_context and consume_context, you can easily reach up to modify that state:

src/guide_state.rs

#[component]
fn Player() -> Element {
    rsx! {
        button {
            onclick: move |_| consume_context::<MusicPlayer>().song.set("Vienna".to_string()),
            "Shuffle"
        }
    }
}
Any components that read the song signal will automatically re-render when the value changes.

Global Signals
Occasionally you'll want a simple global value. This is where GlobalSignal helps. GlobalSignals are a combination of the Context system and Signals that require no additional structs or setup.

Simply declare a GlobalSignal somewhere in your app:

src/guide_state.rs

static SONG: GlobalSignal<String> = Signal::global(|| "Drift Away".to_string());
And then read and write to it from anywhere:

src/guide_state.rs

#[component]
fn Player() -> Element {
    rsx! {
        h3 { "Now playing {SONG}" }
        button {
            onclick: move |_| *SONG.write() = "Vienna".to_string(),
            "Shuffle"
        }
    }
}
📣 GlobalSignals are only global to one app - not the entire program. On the server, every app gets its own GlobalSignal.

We won't need either GlobalSignal or Context for HotDog, but it's important to know that these are available to you.




Fetching Data
Our HotDog app has some basic interactivity but does not yet fetch new dog images. In this chapter, we'll interact with async and fetching data from an API.

Adding Dependencies
Dioxus does not provide any built-in utilities for fetching data. Crates like dioxus-query exist, but for this tutorial we'll implement data-fetching from scratch.

First, we need to add two new dependencies to our app: serde and reqwest.

Reqwest provides an HTTP client for fetching.
Serde will let us derive a JSON Deserializer to decode the response.
In a new terminal window, add these crates to your app with cargo add.


cargo add reqwest --features json
cargo add serde --features derive
Defining a Response Type
We'll be using the amazing dog.ceo/dog-api to fetch images of dogs for HotDog. Fortunately, the API response is quite simple to deserialize.

Let's create a new Rust struct that matches the format of the API and derive Deserialize for it.

The Dog API docs outline a sample API response:


{
    "message": "https://images.dog.ceo/breeds/leonberg/n02111129_974.jpg",
    "status": "success"
}
Our Rust struct needs to match that format, though for now we'll only include the "message" field.

src/guide_data_fetching.rs

#[derive(serde::Deserialize)]
struct DogApi {
    message: String,
}
Using reqwest and async
Dioxus has stellar support for asynchronous Rust. We can simply convert our onclick handler to be async and then set the img_src after the future has resolved.


The changes to our code are quite simple - just add the reqwest::get call and then call .set() on img_src with the result.

src/guide_data_fetching.rs

#[component]
fn DogView() -> Element {
    let mut img_src = use_signal(|| "".to_string());

    let fetch_new = move |_| async move {
        let response = reqwest::get("https://dog.ceo/api/breeds/image/random")
            .await
            .unwrap()
            .json::<DogApi>()
            .await
            .unwrap();

        img_src.set(response.message);
    };

    // ..

    rsx! {
        div { id: "dogview",
            img { src: "{img_src}" }
        }
        div { id: "buttons",
            // ..
            button { onclick: fetch_new, id: "save", "save!" }
        }
    }
}
Dioxus automatically calls spawn on asynchronous closures. You can also use spawn to perform async work without async closures - just call spawn() on any async block.

src/guide_data_fetching.rs

rsx! {
    button {
        onclick: move |_| {
            spawn(async move {
                // do some async work...
            });
        }
    }
}
The futures passed to spawn must not contain latent references to data outside the async block. Data that is Copy can be captured by async blocks, but all other data must be moved, usually by calling .clone().

Managing Data Fetching with use_resource
Eventually, using bare async calls might lead to race conditions and weird state bugs. For example, if the user clicks the fetch button too quickly, then two requests will be made in parallel. If the request is updating data somewhere else, the wrong request might finish early and causes a race condition.

In Dioxus, Resources are pieces of state whose value is dependent on the completion of some asynchronous work. The use_resource hook provides a Resource object with helpful methods to start, stop, pause, and modify the asynchronous state.

Let's change our component to use a resource instead:

src/guide_data_fetching.rs

#[component]
fn DogView() -> Element {
    let mut img_src = use_resource(|| async move {
        reqwest::get("https://dog.ceo/api/breeds/image/random")
            .await
            .unwrap()
            .json::<DogApi>()
            .await
            .unwrap()
            .message
    });

    rsx! {
        div { id: "dogview",
            img { src: img_src.cloned().unwrap_or_default() }
        }
        div { id: "buttons",
            button { onclick: move |_| img_src.restart(), id: "skip", "skip" }
            button { onclick: move |_| img_src.restart(), id: "save", "save!" }
        }
    }
}
Resources are very powerful: they integrate with Suspense, Streaming HTML, reactivity, and more.

The details of the Resource API are not terribly important right now, but you'll be using Resources frequently in larger apps, so it's a good idea to read the docs.


Adding a Backend
Dioxus is a fullstack framework, meaning it allows you to seamlessly build your frontend alongside your backend.

We provide a number of utilities like Server Functions, Server Futures, and Server State for you to integrate into your apps. In this chapter, we'll cover loading and saving state to our backend with Server Functions. For an in-depth guide on fullstack, check out the dedicated Fullstack Guide.

Enabling Fullstack
Before we can start using server functions, we need to enable the "fullstack" feature on Dioxus in our Cargo.toml.


[dependencies]
dioxus = { version = "0.6.0", features = ["fullstack"] }
We also need to add the "server" feature to our app's features in the Cargo.toml and remove the default web target.


[features]
default = [] # <----- remove the default web target
web = ["dioxus/web"]
desktop = ["dioxus/desktop"]
mobile = ["dioxus/mobile"]
server = ["dioxus/server"] # <----- add this additional target
If you selected yes to the "use fullstack?" prompt when creating your app, you will already have this set up!

📣 Unfortunately, dx doesn't know how to hot-reload this change, so we'll need to kill our currently running dx serve process and launch it again.

Now instead of running dx serve, you need to run with a manual platform with dx serve --platform web. Give your app a moment to build again and make sure that the "fullstack" feature is enabled in the dashboard.

Fullstack Enabled

Server Functions: an inline RPC system
Dioxus integrates with the server_fn crate to provide a simple inline communication system for your apps. The server_fn crate makes it easy to build your app's backend with just basic Rust functions. Server Functions are async functions annotated with the #[server] attribute.

A typical server function looks like this:

src/guide_backend.rs

#[server]
async fn save_dog(image: String) -> Result<(), ServerFnError> {
    Ok(())
}
Every server function is an async function that takes some parameters and returns a Result<(), ServerFnError>. Whenever the client calls the server function, it sends an HTTP request to a corresponding endpoint on the server. The parameters of the server function are serialized as the body of the HTTP request. As a result, each argument must be serializable.

On the client, the server function expands to a reqwest call:

src/guide_backend.rs

// on the client:
async fn save_dog(image: String) -> Result<(), ServerFnError> {
    reqwest::Client::new()
        .post("http://localhost:8080/api/save_dog")
        .json(&image)
        .send()
        .await?;
    Ok(())
}
On the server, the server function expands to an axum handler:

src/guide_backend.rs

// on the server:
struct SaveDogArgs {
    image: String,
}

async fn save_dog(Json(args): Json<SaveDogArgs>) -> Result<(), ServerFnError> {
    Ok(())
}
When dioxus::launch is called, the server functions are automatically registered for you and set up as an Axum router.

src/guide_backend.rs

async fn launch(config: ServeConfig, app: fn() -> Element) {
    // register server functions
    let router = axum::Router::new().serve_dioxus_application(config, app);

    // start server
    let socket_addr = dioxus_cli_config::fullstack_address_or_localhost();
    let listener = tokio::net::TcpListener::bind(socket_addr).await.unwrap();
    axum::serve(listener, router).await.unwrap();
}
As of Dioxus 0.6, we only support the axum server framework. We plan to build additional server features in the future and only support axum to ship faster.

In some cases, the dioxus::launch function might be too limiting for your use-case on the server. You can easily drop down to axum by changing your main.rs. The dioxus::launch function also handles setting up logging and reading environment variables, which you will have to handle yourself.

src/guide_backend.rs

fn main() {
    #[cfg(feature = "server")]
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(launch_server());
    #[cfg(not(feature = "server"))]
    dioxus::launch(App);
}

#[cfg(feature = "server")]
async fn launch_server() {
    // Connect to dioxus' logging infrastructure
    dioxus::logger::initialize_default();

    // Connect to the IP and PORT env vars passed by the Dioxus CLI (or your dockerfile)
    let socket_addr = dioxus_cli_config::fullstack_address_or_localhost();

    // Build a custom axum router
    let router = axum::Router::new()
        .serve_dioxus_application(ServeConfigBuilder::new(), App)
        .into_make_service();

    // And launch it!
    let listener = tokio::net::TcpListener::bind(socket_addr).await.unwrap();
    axum::serve(listener, router).await.unwrap();
}
The Client/Server split
When Dioxus builds your fullstack apps, it actually creates two separate applications: the server and the client. To achieve this, dx passes different features to each build.

The client is built with --feature web
The server is built with --feature server
Server Client Split

When embedding server code in our apps, we need to be careful about which code gets compiled. The body of the server function is designed to only be executed on the server - not the client. Any code configured by the "server" feature will not be present in the final app. Conversely, any code not configured by the "server" feature will make it into the final app.

src/guide_backend.rs

// ❌ this will leak your DB_PASSWORD to your client app!
static DB_PASSWORD: &str = "1234";

#[server]
async fn DoThing() -> Result<(), ServerFnError> {
    connect_to_db(DB_PASSWORD).await
    // ...
}
Instead, we recommend placing server-only code within modules configured for the "server" feature.

src/guide_backend.rs

// ✅ code in this module can only be accessed on the server
#[cfg(feature = "server")]
mod server_utils {
    pub static DB_PASSWORD: &str = "1234";
}
In addition to the "server" feature, Dioxus expects a client side rendering feature like "web" or "desktop". Some libraries like web-sys only work when running in the browser, so make sure to not run specific client code in your server functions or before your launch. You can place client only code under a config for a client target feature like "web".

src/guide_backend.rs

fn main() {
    // ❌ attempting to use web_sys on the server will panic!
    let window = web_sys::window();

    // ..

    dioxus::launch(App);
}
Managing Dependencies
Some dependencies like Tokio only compile properly when targeting native platforms. Other dependencies like jni-sys only work properly when running on a specific platform. In these cases, you'll want to make sure that these dependencies are only compiled when a particular feature is enabled. To do this, we can use Rust's optional flag on dependencies in our Cargo.toml.


[dependencies]
tokio = { version = "1", optional = true }

[features]
default = []
server = ["dep:tokio"]
Eventually, if your project becomes large enough, you might want to pull your server functions out into their own crate to be used across different apps. We'd create a server crate in our workspace:


├── Cargo.toml
└── crates
    ├── dashboard
    ├── marketplace
    └── server
And then we'd import the server functions in our app, disabling their "server" feature.


[dependencies]
server = { workspace = true, default-features = false }
We provide a longer guide about the details of managing dependencies across the server and the client here.

Our HotDog Server Function
Revisiting our HotDog app, let's create a new server function that saves our favorite dog to a file called dogs.txt. In production, you'd want to use a proper database as covered in the next chapter, but for now we'll use a simple file to test things out.

src/guide_backend.rs

// Expose a `save_dog` endpoint on our server that takes an "image" parameter
#[server]
async fn save_dog(image: String) -> Result<(), ServerFnError> {
    use std::io::Write;

    // Open the `dogs.txt` file in append-only mode, creating it if it doesn't exist;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .append(true)
        .create(true)
        .open("dogs.txt")
        .unwrap();

    // And then write a newline to it with the image url
    file.write_fmt(format_args!("{image}\n"));

    Ok(())
}
Calling the server function
Now, in our client code, we can actually call the server function.

src/guide_backend.rs

fn DogView() -> Element {
    let mut img_src = use_resource(snipped!());

    // ...
    rsx! {
        // ...
        div { id: "buttons",
            // ...
            button {
                id: "save",
                onclick: move |_| async move {
                    let current = img_src.cloned().unwrap();
                    img_src.restart();
                    _ = save_dog(current).await;
                },

                "save!"
            }
        }
    }
}
}
Wow, our app is really coming together!


Server functions are extremely capable and can even be used during server-side-rendering. Check out the complete fullstack guide for more information.

Adding More Routes
So far, our app has only had a single page. Let's change that!

In this chapter, we'll be adding a Navbar, a welcome screen, and a "favorites" page where we can revisit our favorite dogs.

Organizing our Project
Before we get too far with adding new pages to our app, let's organize our codebase a bit better. For larger projects you might want to break your app into different smaller crates. For HotDog, we'll keep it simple.

The dx new Jumpstart and Workspace templates provide great scaffolding for new apps!

We generally recommend splitting your components, models, and backend functionality into different files. For HotDog, we're going to use a simple directory structure:


├── Cargo.toml
├── assets
│   └── main.css
└── src
    ├── backend.rs
    ├── components
    │   ├── favorites.rs
    │   ├── mod.rs
    │   ├── nav.rs
    │   └── view.rs
    └── main.rs
We'll have a backend.rs that contains our server functions and a components folder that contains our components. We don't have a NavBar or a Favorites component yet, but we'll still create the relevant files before adding them. By splitting out our server functions into a backend.rs file, we'll make it easier to extract our backend functionality as a shared library for different apps in the future.

Our components/mod.rs file will simply import and re-export the components in view.rs, nav.rs, and favorites.rs:


mod favorites;
mod nav;
mod view;

pub use favorites::*;
pub use nav::*;
pub use view::*;
Finally, we need to bring backend and components into scope in our main.rs file:


mod components;
mod backend;

use crate::components::*;
For more information on organizing Rust projects with modules, see the Modules section of the Rust Book.

Creating a Route
Most Dioxus apps you'll build will have different screens. This could include pages like Login, Settings, and Profile. Our HotDog app will have two screens: a DogView page and a Favorites page.

Dioxus provides a first-party router that natively integrates with web, desktop, and mobile. For example, on web, whenever you visit the /favorites url in your browser, the corresponding Favorites page will load. The Dioxus router is very powerful, and most importantly, type-safe. You can rest easy knowing that users will never be sent to an invalid route. To achieve this, we first need to add the "Router" feature to the Cargo.toml file:


[dependencies]
dioxus = { version = "0.6.0", features = ["fullstack", "router"] } # <----- add "router"
Next, the Dioxus router is defined as an enum with the Routable derive attribute:

src/guide_router.rs

#[derive(Routable, Clone, PartialEq)]
enum Route {
    #[route("/")]
    DogView,
}
With the Dioxus router, every route is an enum variant with a #[route] attribute that specifics the route's URL. Whenever the router renders our route, the component of the same name will be rendered.

src/guide_router.rs

use dioxus::prelude::*;

#[derive(Routable, Clone, PartialEq)]
enum Route {
    #[route("/")]
    DogView, // <---- a DogView component must be in scope
}

fn DogView() -> Element {
    todo!()
}
Rendering the Route
Now that we have our app's Route defined, we need to render it. Let's change our app component to render the Route {} component instead of the DogView.

src/guide_router.rs

fn app() -> Element {
    rsx! {
        document::Stylesheet { href: asset!("/assets/main.css") }

        // 📣 delete Title and DogView and replace it with the Router component.
        Router::<Route> {}
    }
}
When the Router {} component renders, it will parse the document's current URL into a Route variant. If the url doesn't parse properly, the router will render nothing unless you add a "catch-all" route:

src/guide_router.rs

#[derive(Routable, Clone, PartialEq)]
enum Route {
    // ...
    // We can collect the segments of the URL into a Vec<String>
    #[route("/:..segments")]
    PageNotFound { segments: Vec<String> },
}
Note here that the PageNotFound route takes the "segments" parameter. Dioxus routes are not only type-safe as variants, but also type-safe with URL parameters. For more information on how this works, check the router guide.

At this point, we should see our app, but this time without its Title.

No Navbar

Rendering the NavBar with a Layout
We're rendering our DogView component, but unfortunately we no longer see our Title. Let's add that back and turn it into a NavBar!

In our src/components/nav.rs file, we'll add back our Title code, but rename it to NavBar and modify it with two new items: the Link {} and Outlet components.

src/guide_router.rs

use crate::Route;
use dioxus::prelude::*;

#[component]
pub fn NavBar() -> Element {
    rsx! {
        div { id: "title",
            Link { to: Route::DogView,
                h1 { "🌭 HotDog! " }
            }
        }
        Outlet::<Route> {}
    }
}
The Link {} component wraps the anchor <a> element with a type-safe interface. This means any struct that implements Routable - anything that can .to_string() - is a valid navigation target.

src/guide_router.rs

// Using the Link with Route
Link { to: Route::DogView }

// Or passing in a "/" route directly
Link { to: "/" }
The Link component takes many different arguments, making it possible to extend and customize for your use-case.

In NavBar, we also added an Outlet::<Route> {} component. When the Router component renders, it first looks for any child Outlet components. If one is present, it renders the current route under the outlet. This lets us wrap the current page in extra elements - in this case, the NavBar. If no Outlet is present, then the current route is simply rendered where the Router {} is declared.

To actually add the NavBar component to our app, we need to update our Route enum with the #[layout] attribute. This forces the router to render the NavBar component first so it can expose its Outlet {}.

src/guide_router.rs

#[derive(Routable, PartialEq, Clone)]
enum Route {
    #[layout(NavBar)] // <---- add the #[layout] attribute
    #[route("/")]
    DogView,
}
The layout attribute instructs the Router to wrap the following enum variants in the given component.


Router  {
    NavBar {
        Outlet {
            if route == “/” {
                DogView {}
            }
        }
    }
}
Visually, this should be straight-forward to understand. Note that the Router and Outlet share the same Route generic type.

RouterLayout

Adding a Favorites Route
Now that we understand the fundamentals of routing, let's finally add our Favorites page so we can view the dog photos we saved.

We'll start by creating an empty component src/components/favorites.rs:

src/guide_router.rs

use dioxus::prelude::*;

#[component]
pub fn Favorites() -> Element {
    rsx! { "favorites!" }
}
And then let's make sure to add a new variant in our Route enum:

src/guide_router.rs

#[derive(Routable, PartialEq, Clone)]
enum Route {
    #[layout(NavBar)]
    #[route("/")]
    DogView,

    #[route("/favorites")]
    Favorites, // <------ add this new variant
}
To make sure the user can reach this page, let's also add a button in the nav that points to it.

src/guide_router.rs

use crate::Route;
use dioxus::prelude::*;

#[component]
pub fn NavBar() -> Element {
    rsx! {
        div { id: "title",
            Link { to: Route::DogView,
                h1 { "🌭 HotDog! " }
            }
            Link { to: Route::Favorites, id: "heart", "♥️" } // <------- add this Link
        }
        Outlet::<Route> {}
    }
}
Our Favorites Page
Finally, we can build our favorites page. Let's add a new list_dogs server function that fetches the 10 most recently saved dog photos:

src/guide_router.rs

// Query the database and return the last 10 dogs and their url
#[server]
pub async fn list_dogs() -> Result<Vec<(usize, String)>, ServerFnError> {
    let dogs = DB.with(|f| {
        f.prepare("SELECT id, url FROM dogs ORDER BY id DESC LIMIT 10")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    });

    Ok(dogs)
}
Now, we can fill in our component. We're going to use the same use_resource hook from earlier. Resolving the request from the server might take some time, so we'll use the .suspend()? method on Resource to wait for the request to finish before mapping the contents to a list.

src/guide_router.rs

use dioxus::prelude::*;

#[component]
pub fn Favorites() -> Element {
    // Create a pending resource that resolves to the list of dogs from the backend
    // Wait for the favorites list to resolve with `.suspend()`
    let mut favorites = use_resource(super::backend::list_dogs).suspend()?;

    rsx! {
        div { id: "favorites",
            div { id: "favorites-container",
                for (id, url) in favorites().unwrap() {
                    // Render a div for each photo using the dog's ID as the list key
                    div {
                        key: id,
                        class: "favorite-dog",
                        img { src: "{url}" }
                    }
                }
            }
        }
    }
}
As a stretch goal, try adding a button that lets the user also delete items from the database.



Bundling
Congratulations! You built your first fully-functional Dioxus app, completely loaded with Routing, asynchronous data-fetching, Server Functions, and a database! That's incredible for just a few minutes of work.

Let's get your app bundled for multiple platforms and then ready to deploy.

Testing on Desktop and Mobile
So far, we've been testing our app in a simple web browser. Let's actually build and test our app for mobile platforms.

In Dioxus 0.6, dx finally supports dx serve for Android and iOS!

Testing on iOS
To test iOS, your development environment needs to be setup to build iOS apps. This involves a few steps:

Make sure you are developing on a device running macOS
Install XCode
Download a recent iOS SDK and Emulator pack
Install the iOS Rust toolchains (aarch64-apple-ios aarch64-apple-ios-sim)
This is a multi-step process and requires creating an Apple Developer account. You shouldn't need to pay any fees until you want to sign your app. Signing your app is required for deploying to the Apple App Store and testing on your iOS device.

If everything is installed properly, you should be able to open the Simulator app:


open /Applications/Xcode.app/Contents/Developer/Applications/Simulator.app
If the Simulator app opens but no device pops up, you might need to open a specific device. Use xcrun to discover which devices you have installed.


xcrun simctl list
Identify an available device. We're going to simulate an iPhone 15 Pro Max:


xcrun simctl boot "iPhone 15 Pro Max"
Once the simulator is booted, we can run dx serve --platform ios.


Fantastic - our app works seamlessly with no changes.

Testing on Android
Setting up your environment for Android development takes time, so make sure to read the mobile tooling guide.

Install the Android NDK and SDK
Set JAVA_HOME, ANDROID_HOME, NDK_HOME, and fix PATH issues to use the emulator tool
Install and set up an Android emulator
Install the Android rustup targets (aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android)
Let's start an emulator. We can use the emulator command which should be in your PATH if setup properly. We're going to use our Pixel_6_API_34 emulator, but you can use any device you've configured.


emulator -avd Pixel_6_API_34  -netdelay none -netspeed full
If we try to dx serve --platform android, we'll find that our app fails to build for Android. This is not good!


12:45:39 [cargo]   Could not find directory of OpenSSL installation, and this `-sys` crate cannot
12:45:39 [cargo]   proceed without this knowledge. If OpenSSL is installed and this crate had
12:45:39 [cargo]   trouble finding it,  you can set the `OPENSSL_DIR` environment variable for the
12:45:39 [cargo]   compilation process.
12:45:39 [cargo]   Make sure you also have the development packages of openssl installed.
12:45:39 [cargo]   For example, `libssl-dev` on Ubuntu or `openssl-devel` on Fedora.
12:45:39 [cargo]   If you're in a situation where you think the directory *should* be found
12:45:39 [cargo]   automatically, please open a bug at https://github.com/sfackler/rust-openssl
12:45:39 [cargo]   and include information about your system as well as this message.
12:45:39 [cargo]   $HOST = aarch64-apple-darwin
12:45:39 [cargo]   $TARGET = aarch64-linux-android
12:45:39 [cargo]   openssl-sys = 0.9.104
Currently, rust-openssl does not cross-compile properly for Android targets. To fix this, we need to add the openssl crate to our Cargo.toml and then enable its "vendored" feature. This will build OpenSSL from source instead of trying and failing to read it from the Android NDK.

We're only going to enable the vendored feature when targeting Android.


[target.'cfg(target_os = "android")'.dependencies]
openssl = { version = "0.10", features = ["vendored"] }
In the future, Dioxus might add OpenSSL's vendored feature implicitly to make this error go away. We're covering it here since it's important to understand that not every Rust dependency works out-of-the-box for iOS and Android. Unfortunately, the Rust ecosystem for mobile is still quite young and you'll need to know how to solve problems like these.

Let's try again!


dx serve --platform android

Testing on Desktop
HotDog also works on macOS, Windows, and Linux! We can use dx serve --platform desktop to serve our app as a desktop app.

HotDogDesktop

Bundling for the web
After we're done making changes to our server and client apps, we can build bundles that are ready to distribute.

We're going to follow the same pattern as dx serve but with dx bundle. To start, let's build the web version of our app.


dx bundle --platform web
We should receive a series of INFO traces from the CLI as it builds, and then finally a path to the public folder it generates. Let's cd into its public directory and then check out its parent directory (cd ..) (the "web" folder).


❯ tree -L 3 --gitignore
.
├── public
│   ├── assets
│   │   ├── favicon.ico
│   │   ├── header.svg
│   │   ├── main-14aa55e73f669f3e.css
│   │   ├── main.css
│   │   └── screenshot.png
│   ├── index.html
│   └── wasm
│       ├── hot_dog.js
│       ├── hot_dog.js.br
│       ├── hot_dog_bg.wasm
│       ├── hot_dog_bg.wasm.br
│       └── snippets
└── server
dx built a public folder containing our assets, index.html, and various JavaScript snippets. Alongside our public folder is a server binary. When we deploy our web assets, we'll also want to deploy the server since it provides our server functions.

We can manually run the server simply by executing it. If you're using a default dioxus::launch setup, then the server will read the IP and PORT environment variables to serve.

📣 If you intend to serve from within a container (e.g., Docker), then you need to override the default 127.0.0.1 address with IP=0.0.0.0 to listen for external connections.

Serving the server

Bundling for Desktop and Mobile
To bundle desktop and mobile apps for deployment, we'll again use dx bundle. As of today, dx bundle only builds desktop apps for the native platform and architecture. Unfortunately, you can't build macOS apps from Windows, Linux apps from Mac, etc. We recommend using a Continuous Integration Matrix (like Github Actions) to perform a "cross-build" of your app in multiple different containers.

When bundling installable apps, there are many distribution formats to choose from. We can specify these formats using the --package-types flag on dx bundle. Dioxus supports packaging a broad number of package types:

macOS: .app, .dmg
Linux: .appimage, .rpm, .deb
Windows: .msi, .exe
iOS: .app
Android: .apk
You can specify package types like so:


dx bundle --platform desktop \
    --package-types "macos" \
    --package-types "dmg"
Note that not all package-types are compatible with each platform - eg. only .exe can be built when specifying --platform desktop.

We should see the outputs in our terminal:


18.252s  INFO Bundled app successfully!
18.252s  INFO App produced 2 outputs:
18.252s  INFO app - [/Users/jonkelley/Development/Tinkering/06-demos/hot_dog/target/dx/hot_dog/bundle/macos/bundle/macos/HotDog.app]
18.252s  INFO dmg - [/Users/jonkelley/Development/Tinkering/06-demos/hot_dog/target/dx/hot_dog/bundle/macos/bundle/dmg/HotDog_0.1.0_aarch64.dmg]
Generally, you can distribute desktop apps without needing an app store. However, some platforms like macOS might require you to sign and notarize your application to be considered "safe" for your users to open.

When distributing mobile apps, you are required to sign and notarize your apps. Currently, Dioxus doesn't provide built-in utilities for this, so you'll need to figure out signing by reading 3rd-party documentation.

Tauri provides documentation on the signing process:

macOS
iOS
Android
Windows
Linux
Customizing your Bundle
Before you ship your app, you might want to configure how your app icon looks, what entitlements it has, and other details. Our dx bundle tool can help you configure your bundles in a variety of ways.

To configure our bundle, we'll use our Dioxus.toml and modify the bundle section.


[application]
name = "docsite"

[bundle]
identifier = "com.dioxuslabs"
publisher = "DioxusLabs"
icon = ["assets/icon.png"]
For a full list of options, see the reference page on the bundle section.

Automating dx bundle with JSON mode
Also added in Dioxus 0.6 is a JSON output mode for dx. This makes it possible to parse the output of the CLI using tools like jq which provide stdin/stdout support for JSON parsing.

This mode is not particular friendly to humans, but does contain more information than the standard trace output.


{"timestamp":"   9.927s","level":"INFO","message":"Bundled app successfully!","target":"dx::cli::bundle"}
{"timestamp":"   9.927s","level":"INFO","message":"App produced 2 outputs:","target":"dx::cli::bundle"}
{"timestamp":"   9.927s","level":"DEBUG","message":"Bundling produced bundles: [\n    Bundle {\n        package_type: MacOsBundle,\n        bundle_paths: [\n            \"/Users/jonkelley/Development/Tinkering/06-demos/hot_dog/target/dx/hot_dog/bundle/macos/bundle/macos/HotDog.app\",\n        ],\n    },\n    Bundle {\n        package_type: Dmg,\n        bundle_paths: [\n            \"/Users/jonkelley/Development/Tinkering/06-demos/hot_dog/target/dx/hot_dog/bundle/macos/bundle/dmg/HotDog_0.1.0_aarch64.dmg\",\n        ],\n    },\n]","target":"dx::cli::bundle"}
{"timestamp":"   9.927s","level":"INFO","message":"app - [/Users/jonkelley/Development/Tinkering/06-demos/hot_dog/target/dx/hot_dog/bundle/macos/bundle/macos/HotDog.app]","target":"dx::cli::bundle"}
{"timestamp":"   9.927s","level":"INFO","message":"dmg - [/Users/jonkelley/Development/Tinkering/06-demos/hot_dog/target/dx/hot_dog/bundle/macos/bundle/dmg/HotDog_0.1.0_aarch64.dmg]","target":"dx::cli::bundle"}
{"timestamp":"   9.927s","level":"DEBUG","json":"{\"BundleOutput\":{\"bundles\":[\"/Users/jonkelley/Development/Tinkering/06-demos/hot_dog/target/dx/hot_dog/bundle/macos/bundle/macos/HotDog.app\",\"/Users/jonkelley/Development/Tinkering/06-demos/hot_dog/target/dx/hot_dog/bundle/macos/bundle/dmg/HotDog_0.1.0_aarch64.dmg\"]}}","target":"dx"}
JSON mode works with all dx commands. However, it is most useful with dx build and dx bundle. The CLI always guarantees that the last emitted line is the result of the command. To collect the list of bundles from the dx bundle command, we can use tail -1 and simple jq.


dx bundle --platform desktop \
    --json-output \
    --verbose \
    | tail -1 \
    | jq -r '.json | fromjson | .BundleOutput.bundles []'
This returns the list of bundles:


/Users/jonkelley/Development/Tinkering/06-demos/hot_dog/target/dx/hot_dog/bundle/macos/bundle/macos/HotDog.app
/Users/jonkelley/Development/Tinkering/06-demos/hot_dog/target/dx/hot_dog/bundle/macos/bundle/dmg/HotDog_0.1.0_aarch64.dmg

Deploying
We're finally ready to deploy our bundled apps into the world. Congrats on making it this far!

This step is optional for the tutorial but worth covering to understand the process. Feel free to skip ahead to next steps if you're not interested in deploying.

Dioxus Deploy
As mentioned in the introduction, Dioxus is an independent project with aspirations to fund itself through a paid deploy platform. Hopefully, one day, enough people ship apps with Dioxus Deploy to fund development on Dioxus itself!

Currently, Dioxus does not provide its own deploy platform. If you want to sign-up for the beta and help us design the ideal "end-to-end app-development experience," please join the waitlist!

Deploy

Deploying your Desktop and Mobile apps
Generally, deploying a desktop app is as simple as distributing the bundle directly. Simply upload your app bundles to a host like GitHub or S3. With a download link, your users can easily download and install your apps.

📣 When shipping fullstack apps to production, you'll want to make sure to set your backend API URL properly as covered later.

If you'd like to distribute your app through app stores, you'll need to follow some additional steps.

iOS: Directly publish to the Apple App Store
macOS: Directly publish to the Apple App Store
Android: Directly publish to the Google Play Store
Tauri provides some helpful guides for deploying Tauri apps which, while not Dioxus apps, need to follow many of the same steps for deploying to app stores.

Making native app distribution easier is a top priority for Dioxus Deploy!

Deploy Requirements
Dioxus web apps are structured as a Client bundle and a Server executable. Generally, any deploy provider that exposes a simple container will be sufficient for a Dioxus fullstack web application.

Some providers like Cloudflare Workers and Fermyon Spin provide WASM-based containers for apps. WASM runtimes are typically cheaper to operate and can horizontally scale better than a traditional virtual-machine based container. When deploying on WASM runtimes, you will need to create a WASM build of your server manually.

Running the webserver is as simple as executing ./server. Make sure to set the IP and PORT environment variables correctly:

Serving a Server

Choosing a deploy provider
There are many deploy providers! We're not going to get too deep into the pros/cons of any particular provider. Generally, providers are good at one of a few categories: price, performance, UI/UX, advanced features, and enterprise requirements.

Depending on your app, you might have strict requirements like SOC2 or HIPAA compliance. Make sure to do your own research for your own use-case.

AWS: Full-featured cloud provider powered by Amazon.
GCP: Full-featured cloud provider powered by Google.
Azure: Full-featured cloud provider powered by Microsoft.
Fly.io: Simple scale-to-zero micro-vm-based cloud with integrated wireguard.
Vercel: Developer-focused cloud built on AWS cloud functions popular with JavaScript frameworks.
Render: A "Modern Heroku" focused on developer experience and simplicity.
Digital Ocean: A cloud built around virtual machines, databases, and storage.
For HotDog we're going to deploy on Fly.io. We like Fly.io for a number of reasons. Most importantly, Fly is built on Amazon's Firecracker project which is entirely written in Rust!

Fly is also quite simple to get started - just log in with either your GitHub account or Google account.

Building a Dockerfile
Some deploy providers have prebuilt solutions for various runtimes. For example, some have dedicated NodeJS and Python runtimes with strict requirements.

With Rust apps, there generally isn't a prebuilt "pack" to target. In these cases, we need to write a simple Dockerfile which compiles and starts our apps.

Our Dockerfile will have three phases. The first phase downloads and caches dependencies so incremental builds stay fast:


FROM rust:1 AS chef
RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json
In the second phase, we use cargo chef to load cached dependencies and preform the build:


FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .

# Install `dx`
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
RUN cargo binstall dioxus-cli --root /.cargo -y --force
ENV PATH="/.cargo/bin:$PATH"

# Create the final bundle folder. Bundle always executes in release mode with optimizations enabled
RUN dx bundle --platform web
Finally, we copy the built "web" folder to the "slim" runtime that serves our app.


FROM chef AS runtime
COPY --from=builder /app/target/dx/hot_dog/release/web/ /usr/local/app

# set our port and make sure to listen for all connections
ENV PORT=8080
ENV IP=0.0.0.0

# expose the port 8080
EXPOSE 8080

WORKDIR /usr/local/app
ENTRYPOINT [ "/usr/local/app/server" ]
It's also a smart idea to set up a .dockerignore file:


**/target
**/dist
LICENSES
LICENSE
temp
README.md
Deploying to Fly
To get started with Fly, we need to go through the Sign Up flow and enter our details. This shouldn't take too long.

We'll add the dockerfile from above along with the dockerignore. We'll want to install flyctl which also installs the fly CLI.

Let's call fly launch which will automatically initialize our fly.toml.

Fly Launch

fly launch will spin up a build machine for us and build our app. In a minute or two, our app should be fully built and deployed.

If we ever want to re-deploy our code, we can run fly deploy.


We can also add a volume to our app to persist our Sqlite database by adding a [mounts] section to our Fly.toml:


[mounts]
  source = "hotdogdb"
  destination = "/usr/local/app/hotdogdb"
Once the build is complete, Fly will assign our app a URL that we can customize later. With any luck, our app should be live!

Live App

Continuous Deployment
Fly also supports continuous deployment. Whenever we push to our GitHub repository, we can execute fly deploy automatically. This can serve as a foundation for staging environments and automatic releases.

Our app just needs a .github/workflows/fly-deploy.yml.


name: Fly Deploy
on:
  push:
    branches:
      - main
jobs:
  deploy:
    name: Deploy app
    runs-on: ubuntu-latest
    concurrency: deploy-group
    steps:
      - uses: actions/checkout@v4
      - uses: superfly/flyctl-actions/setup-flyctl@master
      - run: flyctl deploy --remote-only
        env:
          FLY_API_TOKEN: ${{ secrets.FLY_API_TOKEN }}
Fullstack Desktop and Mobile
Now that our backend is live, we can wire up the API to our native apps. By default, Dioxus doesn't know where to find your API, so you'll need to specify the URL manually by calling server_fn::client::set_server_url.

src/guide_deploy.rs

fn main() {
    #[cfg(not(feature = "server"))]
    server_fn::client::set_server_url("https://hot-dog.fly.dev");

    dioxus::launch(App);
}
Note that as our app changes, the "true" endpoint of our server functions might change. The #[server] macro generates an API endpoint with the form of /api/fetch_dogs-jkhj12 where the trailing data is a unique hash. As we update our server functions, the hash will change.

To make server functions maintain a stable endpoint, we can manually name them with the endpoint = "xyz" attribute.

src/guide_deploy.rs

#[server(endpoint = "list_dogs")]
pub async fn list_dogs() -> Result<Vec<(usize, String)>, ServerFnError> {
    todo!()
}

#[server(endpoint = "remove_dog")]
pub async fn remove_dog(id: usize) -> Result<(), ServerFnError> {
    todo!()
}

#[server(endpoint = "save_dog")]
pub async fn save_dog(image: String) -> Result<(), ServerFnError> {
    todo!()
}
Let's re-deploy our web app with fly deploy. This deploy should complete faster thanks to cargo chef caching our build.

Now, with dx serve --platform desktop, we should be able to interact with the same backend across web and desktop.

Amazing! Our startup is coming along nicely.Building UIs with RSX
Dioxus renders to HTML, if you are not familiar with HTML, this guide will help you get started with the basics. For more detail, the MDN docs are a great resource.

Text Nodes
Any content surrounded by quotes is rendered as a text node in rsx:

src/building_uis_with_rsx.rs

rsx! {
    "Hello world"
}
Hello world
You can include formatted segments inside of the text just like the format! macro:

src/building_uis_with_rsx.rs

let user = use_signal(|| User {
    name: "Dioxus".to_string(),
});
rsx! {
    // Unlike the format macro, you can include many expressions inline in the formatted text
    "Hello {user.read().name}"
}
Hello Dioxus
Elements
The most basic building block of HTML is an element. In rsx, you can create elements with the name and then curly braces. One of the most common elements is the input element. The input element creates an interactive input box:

src/building_uis_with_rsx.rs

rsx! {
    input {}
}
Bonus: web components Any element with a dash in the name is a web component. Web components are rendered directly in dioxus without type checking. We recommend wrapping web components in a type safe component to make them easier to use.

src/building_uis_with_rsx.rs

rsx! {
    my-web-component {}
}
Attributes
Attributes provide extra information about an element. You can specify attributes in dioxus inside an element's braces by typing the name of the attribute, a colon, and then the value (typically a formatted string). We can use an attribute to set the type of an input element. The default type is text which shows a text input box, but we can set it to number to only accept numbers:

src/building_uis_with_rsx.rs

rsx! {
    input { type: "number" }
}
Just like text nodes, attributes can include formatted segments. We can set the value of the input element to a signal to control it:

src/building_uis_with_rsx.rs

let mut value = use_signal(|| "Hello world".to_string());
rsx! {
    input { value: "{value}" }
}
Hello world
Conditional Attributes
You can conditionally set an attribute by setting the attribute value to an unterminated if statement. If the if statement evaluates to true, the attribute will be set:

src/building_uis_with_rsx.rs

let number_type = use_signal(|| false);
rsx! {
    input { type: if number_type() { "number" } }
}
Event Listeners
Event listeners allow you to respond to user input. In rsx, event handlers always start with on. The syntax is the same as normal attributes, but event handlers only accept a closure that responds to the event. We can attach an event listener to the oninput event of the input element to listen for changes to the input:

src/building_uis_with_rsx.rs

let mut value = use_signal(|| "Hello world".to_string());
rsx! {
    input {
        oninput: move |event| value.set(event.value()),
        value: "{value}"
    }
}
Hello world
Children
You can add children to an element after all attributes and event listeners. Elements can accept text, components or other elements as children. We can add a div element around our input to center it:

src/building_uis_with_rsx.rs

rsx! {
    div {
        // display sets the layout mode of the element
        display: "flex",
        // justify-content centers the element horizontally
        justify_content: "center",
        input {
            type: "number"
        }
    }
}
Loops
You can insert for loops directly in rsx. The body of the loop accepts any number of children that will be rendered with each iteration of the loop. The ul element in html renders an unordered list with any number of li (list item) elements. We can use those two elements to render a list of items in a loop:

src/building_uis_with_rsx.rs

let mut items = use_signal(|| vec!["Hello", "Dioxus"]);

rsx! {
    ul {
        for item in items.iter() {
            li { "{item}" }
        }
    }
}
Hello
Dioxus
Each item in your list should have unique value that is stable across rerenders called a key. Keys are used to identify how items move while diffing. Without keys, it is easy to accidentally lose or move state when you reorder items in a list. We can add keys to our list items by using the key attribute:

src/building_uis_with_rsx.rs

let mut items = use_signal(|| vec!["Hello", "Dioxus"]);

rsx! {
    ul {
        for item in items.iter() {
            li { key: "{item}", "{item}" }
        }
    }
}
Hello
Dioxus
If Statements
You can also use if/else statements in rsx. Each branch of the if statement accepts child nodes that will be rendered if the condition is true. We can use the if statement to conditionally render a login screen:

src/building_uis_with_rsx.rs

let logged_in = use_signal(|| false);

rsx! {
    div {
        if logged_in() {
            "You are logged in"
        } else {
            "You are not logged in"
        }
    }
}
You are not logged in
Why RSX and not HTML ?
If you've seen React's JSX or the html!{} Rust macro, you might be curious as to why Dioxus chose to use its own syntax instead of a syntax that looks more similar to HTML.

A few reasons:

RSX gets token coloring and code-folding without additional tooling
RSX is faster to type since curly braces are auto-closed
Not all RSX is HTML - Dioxus can be used in non-HTML contexts
HTML is not valid Rust - not all HTML can be used in html!{}

Component Lifecycle
Initializing State with use_hook
use_hook lets you create new state for your component. The closure you pass to use_hook will be called once the first time the component is rendered. Every time the component is re-rendered, the value that was created the first run will be re-used.

src/component_lifecycle.rs

fn UseHook() -> Element {
    // The closure that is passed to use_hook will be called once the first time the component is rendered
    let random_number = use_hook(|| {
        let new_random_number = random_number();

        log!("{new_random_number}");

        new_random_number
    });

    rsx! {
        div { "Random {random_number}" }
    }
}
🔄
Random 13.97
Logs
13.97
Rerendering
You can use tracked values to re-render your component whenever a value changes.

src/component_lifecycle.rs

fn Rerenders() -> Element {
    let mut count = use_signal(|| 0);

    log!("Rerendering parent component with {}", *count.peek());

    rsx! {
        button { onclick: move |_| count += 1, "Increment" }
        // Since we read count here, the component will rerender when count changes
        Count { current_count: count() }
    }
}

// If the count prop changes, the component will rerender
#[component]
fn Count(current_count: i32) -> Element {
    log!("Rerendering child component with {current_count}");

    rsx! {
        div { "The count is {current_count}" }
    }
}
Increment
The count is 0
Logs
Rerendering child component with 0
Rerendering parent component with 0
⚠️ Don't mutate state in the body of a component
You should avoid changing state in the body of a component. If you read and write to state in the body of a component, you can cause an infinite loop as the component tries to rerender because of the change which triggers another state change.

src/component_lifecycle.rs

fn Bad() -> Element {
    let mut count = use_signal(|| 0);

    // ❌ Don't mutate state in the body of the component.
    // It can easily cause an infinite loop!
    count += 1;

    rsx! { "{count}" }
}
Instead, derive state with use_memo, use_resource, or mutate state in a effect.

Using Effects
You can use effects to run code whenever a component is rendered.

src/component_lifecycle.rs

fn Effect() -> Element {
    // Effects run after the component is rendered
    // You can use them to read or modify the rendered component
    use_effect(|| {
        log!("Effect ran");
        document::eval(&format!(
            "document.getElementById('effect-output').innerText = 'Effect ran'"
        ));
    });

    rsx! {
        div { id: "effect-output", "This will be changed by the effect" }
    }
}
Effect ran
Logs
Effect ran
Cleaning Up Components with Drop
Before a component is dropped, it will drop all of its hooks. You can use this drop behavior to clean up any resources that your component is using. If you just need the drop effect, you can use the use_drop hook.

src/component_lifecycle.rs

fn TogglesChild() -> Element {
    let mut show = use_signal(|| true);

    rsx! {
        button { onclick: move |_| show.toggle(), "Toggle" }
        if show() {
            Child {}
        }
    }
}

fn Child() -> Element {
    // You can use the use_drop hook to clean up any resources
    use_drop(|| {
        log!("Child dropped");
    });

    rsx! {
        div { "Child" }
    }
}
Toggle
Child
Logs

Managing State
In Dioxus, your app is defined as a function of the current state. As the state changes, the parts of your app that depend on that state will automatically re-run. Reactivity automatically tracks state and updates derived state in your application.

Creating State
You can create mutable state in Dioxus with Signals. Signals are tracked values that automatically update your app when you change them. They form the skeleton of your app's state from which you can derive other state. Signals are often driven directly from user input through event handlers or async tasks.

You can create a signal with the use_signal hook:

src/reactivity.rs

let mut signal = use_signal(|| 0);
Once you have your signal, you can clone it by calling the signal like a function or get a reference to the inner value with the .read() method:

src/reactivity.rs

// Call the signal like a function to clone the current value
let value: i32 = signal();
// get a reference to the inner value with the .read() method
let value: &i32 = &signal.read();
// or use one of the traits implemented for Signal like Display
log!("{signal}");
Finally, you can set the value of the signal with the .set() method or get a mutable reference to the inner value with the .write() method:

src/reactivity.rs

// Set the value from the signal
signal.set(1);
// get a mutable reference to the inner value with the .write() method
let mut value: &mut i32 = &mut signal.write();
*value += 1;
Reactive Scopes
The simplest reactive primitive in Dioxus is the use_effect hook. It creates a closure that is run any time a tracked value that is run inside the closure changes.

Any value you read inside the closure will become a dependency of the effect. If the value changes, the effect will rerun.

src/reactivity.rs

fn Effect() -> Element {
    // use_signal creates a tracked value called count
    let mut count = use_signal(|| 0);

    use_effect(move || {
        // When we read count, it becomes a dependency of the effect
        let current_count = count();
        // Whenever count changes, the effect will rerun
        log!("{current_count}");
    });

    rsx! {
        button { onclick: move |_| count += 1, "Increment" }

        div { "Count is {count}" }
    }
}
Increment
Count is 0
Logs
0
Derived State
use_memo is a reactive primitive that lets you derive state from any tracked value. It takes a closure that computes the new state and returns a tracked value with the current state of the memo. Any time a dependency of the memo changes, the memo will rerun.

The value you return from the closure will only change when the output of the closure changes ( PartialEq between the old and new value returns false).

src/reactivity.rs

fn Memo() -> Element {
    let mut count = use_signal(|| 0);

    // use_memo creates a tracked value that is derived from count
    // Since we read count inside the closure, it becomes a dependency of the memo
    // Whenever count changes, the memo will rerun
    let half_count = use_memo(move || count() / 2);

    use_effect(move || {
        // half_count is itself a tracked value
        // When we read half_count, it becomes a dependency of the effect
        // and the effect will rerun when half_count changes
        log!("{half_count}");
    });

    rsx! {
        button { onclick: move |_| count += 1, "Increment" }

        div { "Count is {count}" }
        div { "Half count is {half_count}" }
    }
}
Increment
Count is 0
Half count is 0
Logs
0
Derived Async State
use_resource is a reactive primitive that lets you derive state from any async closure. It takes an async closure that computes the new state and returns a tracked value with the current state of the resource. Any time a dependency of the resource changes, the resource will rerun.

The value you return from the closure will only change when the state of the future changes. Unlike use_memo, the resource's output is not memoized with PartialEq.

src/reactivity.rs

fn Resource() -> Element {
    let mut count = use_signal(|| 0);

    // use_resource creates a tracked value that is derived from count
    // Since we read count inside the closure, it becomes a dependency of the resource
    // Whenever count changes, the resource will rerun
    let half_count = use_resource(move || async move {
        // You can do async work inside resources
        gloo_timers::future::TimeoutFuture::new(100).await;
        count() / 2
    });

    use_effect(move || {
        // half_count is itself a tracked value
        // When we read half_count, it becomes a dependency of the effect
        // and the effect will rerun when half_count changes
        log!("{:?}", half_count());
    });

    rsx! {
        button { onclick: move |_| count += 1, "Change Signal" }

        div { "Count is {count}" }
        div { "Half count is {half_count():?}" }
    }
}
Change Signal
Count is 0
Half count is Some(0)
Logs
Some(0)
None
Derived UI
Components are functions that return some UI. They memorize the output of the function just like memos. Components keep track of any dependencies you read inside the component and rerun when those dependencies change.

src/reactivity.rs

fn Component() -> Element {
    let mut count = use_signal(|| 0);

    rsx! {
        button { onclick: move |_| count += 1, "Change Signal" }

        // Since we read count inside Component, it becomes a dependency of Component
        // Whenever count changes, Component will rerun
        Count { count: count() }
    }
}

// Components automatically memorize their props. If the props change, Count will rerun
#[component]
fn Count(count: i32) -> Element {
    rsx! {
        div { "Count: {count}" }
    }
}
Change Signal
Count: 0
UI
Count: 0
Working with Untracked State
Most of the state in your app will be tracked values. All built in hooks return tracked values, and we encourage custom hooks to do the same. However, there are times when you need to work with untracked state. For example, you may receive a raw untracked value in props. When you read an untracked value inside a reactive context, it will not subscribe to the value:

src/reactivity.rs

fn Component() -> Element {
    let mut count = use_signal(|| 0);

    rsx! {
        button { onclick: move |_| count += 1, "Change Signal" }

        Count { count: count() }
    }
}

// The count reruns the component when it changes, but it is not a tracked value
#[component]
fn Count(count: i32) -> Element {
    // When you read count inside the memo, it does not subscribe to the count signal
    // because the value is not reactive
    let double_count = use_memo(move || count * 2);

    rsx! {
        div { "Double count: {double_count}" }
    }
}
Change Signal
Double count: 0
UI
Double count: 0
You can start tracking raw state with the use_reactive hook. This hook takes a tuple of dependencies and returns a reactive closure. When the closure is called in a reactive context, it will track subscribe to the dependencies and rerun the closure when the dependencies change.

src/reactivity.rs

#[component]
fn Count(count: i32) -> Element {
    // You can manually track a non-reactive value with the use_reactive hook
    let double_count = use_memo(
        // Use reactive takes a tuple of dependencies and returns a reactive closure
        use_reactive!(|(count,)| count * 2),
    );

    rsx! {
        div { "Double count: {double_count}" }
    }
}
Change Signal
Double count: 0
UI
Double count: 0
Making Props Reactive
To avoid losing reactivity with props, we recommend you wrap any props you want to track in a ReadOnlySignal. Dioxus will automatically convert T into ReadOnlySignal<T> when you pass props to the component. This will ensure your props are tracked and rerun any state you derive in the component:

src/reactivity.rs

// You can track props by wrapping the type in a ReadOnlySignal
// Dioxus will automatically convert T into ReadOnlySignal<T> when you pass
// props to the component
#[component]
fn Count(count: ReadOnlySignal<i32>) -> Element {
    // Then when you read count inside the memo, it subscribes to the count signal
    let double_count = use_memo(move || count() * 2);

    rsx! {
        div { "Double count: {double_count}" }
    }
}
Change Signal
Double count: 0
UI
Double count: 0
Moving Around State
As you create signals and derived state in your app, you will need to move around that state between components. Dioxus provides three different ways to pass around state:

Passing props
You can pass your values through component props. This should be your default when passing around state. It is the most explicit and local to your component. Use this until it gets annoying to pass around the value:

src/moving_state_around.rs

pub fn ParentComponent() -> Element {
    let count = use_signal(|| 0);

    rsx! {
        "Count is {count}"
        IncrementButton {
            count
        }
    }
}

#[component]
fn IncrementButton(mut count: Signal<i32>) -> Element {
    rsx! {
        button {
            onclick: move |_| count += 1,
            "Increment"
        }
    }
}
Count is 0Increment
Passing context
If you need a slightly more powerful way to pass around state, you can use the context API.

The context API lets you pass state from a parent component to all children. This is useful if you want to share state between many components. You can insert a unique type into the context with the use_context_provider hook in the parent component. Then you can access the context in any child component with the use_context hook.

src/moving_state_around.rs

#[derive(Clone, Copy)]
struct MyState {
    count: Signal<i32>,
}

pub fn ParentComponent() -> Element {
    // Use context provider provides an unique type to all children of this component
    let state = use_context_provider(|| MyState {
        count: Signal::new(0),
    });

    rsx! {
        "Count is {state.count}"
        // IncrementButton will have access to the count without explicitly passing it through props
        IncrementButton {}
    }
}

#[component]
fn IncrementButton() -> Element {
    // Use context gets the value from a parent component
    let mut count = use_context::<MyState>().count;

    rsx! {
        button {
            onclick: move |_| count += 1,
            "Increment"
        }
    }
}
Count is 0Increment
This is slightly less explicit than passing it as a prop, but it is still local to the component. This is really great if you want state that is global to part of your app. It lets you create multiple global-ish states while still making state different when you reuse components. If I create a new ParentComponent, it will have a new MyState.

Using globals
Finally, if you have truly global state, you can put your state in a Global<T> static. This is useful if you want to share state with your whole app:

src/moving_state_around.rs

use dioxus::prelude::*;
// Globals are created the first time you access them with the closure you pass to Global::new
static COUNT: GlobalSignal<i32> = Global::new(|| 0);

pub fn ParentComponent() -> Element {
    rsx! {
        "Count is {COUNT}"
        IncrementButton {}
    }
}

fn IncrementButton() -> Element {
    rsx! {
        button {
            // You don't need to pass anything around or get anything out of the context because COUNT is global
            onclick: move |_| *COUNT.write() += 1,
            "Increment"
        }
    }
}
Count is 0Increment
Global state can be very ergonomic if your state is truly global, but you shouldn't use it if you need state to be different for different instances of your component. If I create another IncrementButton it will use the same COUNT. Libraries should generally avoid this to make components more reusable.

Note: Even though it is in a static, COUNT will be different for each app instance so you don't need to worry about state mangling when multiple instances of your app are running on the server

Handling Asynchronous Tasks
Asynchronous tasks are a core part of any modern application. Dioxus provides a few different methods to handle asynchronous tasks. This guide will cover how to use each of them. If you already know what kind of asynchronous task you need, you can skip to the section for that task:

spawn is great for futures you need to run in the background that don't return a value
use_resource handles asynchronous state while retaining control of exactly what happens while the future is running
It can be combined with Suspense to handle many pending tasks with the same loading view
Running Futures with spawn
The spawn method spawns a future in the background and returns a Task that you can use to cancel the future. Spawn is great for futures you want to start and then forget about like sending analytics data to a server:

src/asynchronous.rs

let mut response = use_signal(|| "Click to start a request".to_string());

rsx! {
    button {
        onclick: move |_| {
            response.set("...".into());
            // Spawn will start a task running in the background
            spawn(async move {
                let resp = reqwest::Client::new()
                    .get("https://dioxuslabs.com")
                    .send()
                    .await;

                if resp.is_ok() {
                    response.set("dioxuslabs.com responded!".into());
                } else  {
                    response.set("failed to fetch response!".into());
                }
            });
        },
        "{response}"
    }
}
Click to start a request
Since spawning in event handlers is very common, Dioxus provides a more concise syntax for async event handlers. If you return a future from an event handler, Dioxus will automatically spawn it:

src/asynchronous.rs

let mut response = use_signal(|| "Click to start a request".to_string());

rsx! {
    button {
        // Async closures passed to event handlers are automatically spawned
        onclick: move |_| async move {
            response.set("...".into());
            let resp = reqwest::Client::new()
                .get("https://dioxuslabs.com")
                .send()
                .await;

            if resp.is_ok() {
                response.set("dioxuslabs.com responded!".into());
            } else  {
                response.set("failed to fetch response!".into());
            }
        },
        "{response}"
    }
}

The future you pass to the spawn will automatically be cancelled when the component is unmounted. If you need to keep the future running until it is finished, you can use spawn_forever instead.


Asynchronous State with use_resource
The use_resource can be used to derive asynchronous state. It takes an async closure to calculate the state and returns a tracked value with the current state of the future. Any time a dependency of the resource changes, the resource will rerun:

src/asynchronous.rs

let mut breed = use_signal(|| "hound".to_string());
let dogs = use_resource(move || async move {
    reqwest::Client::new()
        // Since breed is read inside the async closure, the resource will subscribe to the signal
        // and rerun when the breed is written to
        .get(format!("https://dog.ceo/api/breed/{breed}/images"))
        .send()
        .await?
        .json::<BreedResponse>()
        .await
});

rsx! {
    input {
        value: "{breed}",
        // When the input is changed and the breed is set, the resource will rerun
        oninput: move |evt| breed.set(evt.value()),
    }

    div {
        display: "flex",
        flex_direction: "row",
        // You can read resource just like a signal. If the resource is still
        // running, it will return None
        if let Some(response) = &*dogs.read() {
            match response {
                Ok(urls) => rsx! {
                    for image in urls.iter().take(3) {
                        img {
                            src: "{image}",
                            width: "100px",
                            height: "100px",
                        }
                    }
                },
                Err(err) => rsx! { "Failed to fetch response: {err}" },
            }
        } else {
            "Loading..."
        }
    }
}
hound

The use_resource hook might look similar to the use_memo hook, but unlike use_memo, the resource's output is not memoized with PartialEq. That means any components/reactive hooks that read the output will rerun if the future reruns even if the value it returns is the same:

src/asynchronous.rs

let mut number = use_signal(|| 0);

// Resources rerun any time their dependencies change. They will
// rerun any reactive scopes that read the resource when they finish
// even if the value hasn't changed
let halved_resource = use_resource(move || async move { number() / 2 });

log!("Component reran");

rsx! {
    button {
        onclick: move |_| number += 1,
        "Increment"
    }
    p {
        if let Some(halved) = halved_resource() {
            "Halved: {halved}"
        } else {
            "Loading..."
        }
    }
}
Increment
Halved: 0

Logs
Component reran
Component reran
Note: The future you pass to use_resource must be cancel safe. Cancel safe futures are futures that can be stopped at any await point without causing causing issues. For example, this task is not cancel safe:

src/asynchronous.rs

static RESOURCES_RUNNING: GlobalSignal<HashSet<String>> = Signal::global(|| HashSet::new());
let mut breed = use_signal(|| "hound".to_string());
let dogs = use_resource(move || async move {
    // Modify some global state
    RESOURCES_RUNNING.write().insert(breed());

    // Wait for a future to finish. The resource may cancel
    // without warning if breed is changed while the future is running. If
    // it does, then the breed pushed to RESOURCES_RUNNING will never be popped
    let response = reqwest::Client::new()
        .get(format!("https://dog.ceo/api/breed/{breed}/images"))
        .send()
        .await?
        .json::<BreedResponse>()
        .await;

    // Restore some global state
    RESOURCES_RUNNING.write().remove(&breed());

    response
});
RESOURCES_RUNNING:
hound

It can be fixed by making sure the global state is restored when the future is dropped:

src/asynchronous.rs

static RESOURCES_RUNNING: GlobalSignal<HashSet<String>> = Signal::global(|| HashSet::new());
let mut breed = use_signal(|| "hound".to_string());
let dogs = use_resource(move || async move {
    // Modify some global state
    RESOURCES_RUNNING.write().insert(breed());

    // Automatically restore the global state when the future is dropped, even if
    // isn't finished
    struct DropGuard(String);
    impl Drop for DropGuard {
        fn drop(&mut self) {
            RESOURCES_RUNNING.write().remove(&self.0);
        }
    }
    let _guard = DropGuard(breed());

    // Wait for a future to finish. The resource may cancel
    // without warning if breed is changed while the future is running. If
    // it does, then it will be dropped and the breed will be popped
    reqwest::Client::new()
        .get(format!("https://dog.ceo/api/breed/{breed}/images"))
        .send()
        .await?
        .json::<BreedResponse>()
        .await
});
RESOURCES_RUNNING:
hound

Async methods will often mention if they are cancel safe in their documentation.

Unified Loading Views with suspense
SuspenseBoundary is a convenient way to bundle multiple async tasks into a single loading view. It accepts a loading closure and children. You can suspend tasks in children to pause rendering of that child until the future is finished. The suspense boundary will show the loading view instead of the children while any of its children are suspended. Once that suspense is resolved, it will show the children again.

We can use a suspense boundary to show a grid of different breeds of dogs without handling each loading state individually:

src/asynchronous.rs

fn DogGrid() -> Element {
    rsx! {
        SuspenseBoundary {
            // When any child components (like BreedGallery) are suspended, this closure will
            // be called and the loading view will be rendered instead of the children
            fallback: |_| rsx! {
                div {
                    width: "100%",
                    height: "100%",
                    display: "flex",
                    align_items: "center",
                    justify_content: "center",
                    "Loading..."
                }
            },
            div {
                display: "flex",
                flex_direction: "column",
                BreedGallery {
                    breed: "hound"
                }
                BreedGallery {
                    breed: "poodle"
                }
                BreedGallery {
                    breed: "beagle"
                }
            }
        }
    }
}

#[component]
fn BreedGallery(breed: ReadOnlySignal<String>) -> Element {
    let response = use_resource(move || async move {
        // Artificially slow down the request to make the loading indicator easier to seer
        gloo_timers::future::TimeoutFuture::new(1000).await;
        reqwest::Client::new()
            .get(format!("https://dog.ceo/api/breed/{breed}/images"))
            .send()
            .await?
            .json::<BreedResponse>()
            .await
    })
    // Calling .suspend()? will suspend the component and return early while the future is running
    .suspend()?;

    // Then you can just handle the happy path with the resolved future
    rsx! {
        div {
            display: "flex",
            flex_direction: "row",
            match &*response.read() {
                Ok(urls) => rsx! {
                    for image in urls.iter().take(3) {
                        img {
                            src: "{image}",
                            width: "100px",
                            height: "100px",
                        }
                    }
                },
                Err(err) => rsx! { "Failed to fetch response: {err}" },
            }
        }
    }
}
🔄



If you need to change the loading view while a specific task is loading, you can provide a different loading view with the with_loading_placeholder method. The loading placeholder you return from the method will be passed to the suspense boundary and may choose to render it instead of the default loading view:

src/asynchronous.rs

fn DogGrid() -> Element {
    rsx! {
        SuspenseBoundary {
            // The fallback closure accepts a SuspenseContext which contains
            // information about the suspended component
            fallback: |suspense_context: SuspenseContext| if let Some(view) = suspense_context.suspense_placeholder() {
                view
            } else {
                rsx! {
                    div {
                        width: "100%",
                        height: "100%",
                        display: "flex",
                        align_items: "center",
                        justify_content: "center",
                        "Loading..."
                    }
                }
            },
            div {
                display: "flex",
                flex_direction: "column",
                BreedGallery {
                    breed: "hound"
                }
                BreedGallery {
                    breed: "poodle"
                }
                BreedGallery {
                    breed: "beagle"
                }
            }
        }
    }
}

#[component]
fn BreedGallery(breed: ReadOnlySignal<String>) -> Element {
    let response = use_resource(move || async move {
        gloo_timers::future::TimeoutFuture::new(breed().len() as u32 * 100).await;
        reqwest::Client::new()
            .get(format!("https://dog.ceo/api/breed/{breed}/images"))
            .send()
            .await?
            .json::<BreedResponse>()
            .await
    })
    .suspend()
    // You can pass up a loading placeholder to the nearest SuspenseBoundary
    // with the with_loading_placeholder method
    .with_loading_placeholder(move || {
        rsx! {
            div {
                width: "100%",
                height: "100%",
                display: "flex",
                align_items: "center",
                justify_content: "center",
                "Loading {breed}..."
            }
        }
    })?;

    // Then you can just handle the happy path with the resolved future
    rsx! {
        div {
            display: "flex",
            flex_direction: "row",
            match &*response.read() {
                Ok(urls) => rsx! {
                    for image in urls.iter().take(3) {
                        img {
                            src: "{image}",
                            width: "100px",
                            height: "100px",
                        }
                    }
                },
                Err(err) => rsx! { "Failed to fetch response: {err}" },
            }
        }
    }
}
🔄



Suspense with Fullstack
To use suspense in your fullstack application, you need to use the use_server_future hook instead of use_resource. use_server_future handles serialization of the result of the future for hydration. It will also suspend automatically, so you don't need to call .suspend() on the future.

src/asynchronous.rs

#[component]
fn BreedGallery(breed: ReadOnlySignal<String>) -> Element {
    // use_server_future is very similar to use_resource, but the value returned from the future
    // must implement Serialize and Deserialize and it is automatically suspended
    let response = use_server_future(move || async move {
        // The future will run on the server during SSR and then get sent to the client
        reqwest::Client::new()
            .get(format!("https://dog.ceo/api/breed/{breed}/images"))
            .send()
            .await
            // reqwest::Result does not implement Serialize, so we need to map it to a string which
            // can be serialized
            .map_err(|err| err.to_string())?
            .json::<BreedResponse>()
            .await
            .map_err(|err| err.to_string())
        // use_server_future calls `suspend` internally, so you don't need to call it manually, but you
        // do need to bubble up the suspense variant with `?`
    })?;

    // If the future was still pending, it would have returned suspended with the `?` above
    // we can unwrap the None case here to get the inner result
    let response_read = response.read();
    let response = response_read.as_ref().unwrap();

    // Then you can just handle the happy path with the resolved future
    rsx! {
        div {
            display: "flex",
            flex_direction: "row",
            match response {
                Ok(urls) => rsx! {
                    for image in urls.iter().take(3) {
                        img {
                            src: "{image}",
                            width: "100px",
                            height: "100px",
                        }
                    }
                },
                Err(err) => rsx! { "Failed to fetch response: {err}" },
            }
        }
    }
}



Unlike use_resource, use_server_future is only reactive in the closure, not the future itself. If you need to subscribe to another reactive value, you need to read it in the closure before passing it to the future:

src/asynchronous.rs

let id = use_signal(|| 0);
// ❌ The future inside of use_server_future is not reactive
use_server_future(move || {
    async move {
        // But the future is not reactive which means that the future will not subscribe to any reads here
        println!("{id}");
    }
});
// ✅ The closure that creates the future for use_server_future is reactive
use_server_future(move || {
    // The closure itself is reactive which means the future will subscribe to any signals you read here
    let cloned_id = id();
    async move {
        // But the future is not reactive which means that the future will not subscribe to any reads here
        println!("{cloned_id}");
    }
});
When you use suspense with fullstack without streaming enabled, dioxus will wait until all suspended futures are resolved before sending the resolved html to the client. If you enable out of order streaming, dioxus will send the finished HTML chunks to the client one at a time as they are resolved:

src/asynchronous.rs

fn main() {
    dioxus::LaunchBuilder::new()
        .with_context(server_only! {
            // Enable out of order streaming during SSR
            dioxus::fullstack::ServeConfig::builder().enable_out_of_order_streaming()
        })
        .launch(DogGrid);
}

Conclusion
This guide has covered the basics of asynchronous tasks in Dioxus. More detailed documentation about specific hooks are available in docs.rs:

use_resource
use_server_future
SuspenseBoundary
spawn
spawn_forever
More examples of futures and asynchronous tasks are available in the example folder in the dioxus repo.

Assets
Assets are files that are included in the final build of the application. They can be images, fonts, stylesheets, or any other file that is not a source file. Dioxus includes first class support for assets, and provides a simple way to include them in your application and automatically optimize them for production.

Assets in dioxus are also compatible with libraries! If you are building a library, you can include assets in your library and they will be automatically included in the final build of any application that uses your library.

Including images
To include an asset in your application, you can simply wrap the path to the asset in the asset! macro. For example, to include an image in your application, you can use the following code:

src/assets.rs

use dioxus::prelude::*;

fn App() -> Element {
    // You can link to assets that are relative to the package root or even link to an asset from a url
    // These assets will automatically be picked up by the dioxus cli, optimized, and bundled with your final applications
    const ASSET: Asset = asset!("/assets/static/ferrous_wave.png");

    rsx! {
        img { src: "{ASSET}" }
    }
}
You can also optimize, resize, and preload images using the asset! macro. Choosing an optimized file type (like Avif) and a reasonable quality setting can significantly reduce the size of your images which helps your application load faster. For example, you can use the following code to include an optimized image in your application:

src/assets.rs

pub const ENUM_ROUTER_IMG: Asset = asset!(
    "/assets/static/enum_router.png",
    // You can pass a second argument to the asset macro to set up options for the asset
    ImageAssetOptions::new()
        // You can set the image size in pixels at compile time to send the smallest possible image to the client
        .with_size(ImageSize::Manual {
            width: 52,
            height: 52
        })
        // You can also convert the image to a web friendly format at compile time. This can make your images significantly smaller
        .with_format(ImageFormat::Avif)
);

fn EnumRouter() -> Element {
    rsx! {
        img { src: "{ENUM_ROUTER_IMG}" }
    }
}
Including arbitrary files
In dioxus desktop, you may want to include a file with data for your application. If you don't set any options for your asset and the file extension is not recognized, the asset will be copied without any changes. For example, you can use the following code to include a binary file in your application:

src/assets.rs

// You can also collect arbitrary files. Relative paths are resolved relative to the package root
const PATH_TO_BUNDLED_CARGO_TOML: Asset = asset!("/Cargo.toml");
These files will be automatically included in the final build of your application, and you can use them in your application as you would any other file.

Including stylesheets
You can include stylesheets in your application using the asset! macro. Stylesheets will automatically be minified as they are bundled to speed up load times. For example, you can use the following code to include a stylesheet in your application:

src/assets.rs

// You can also bundle stylesheets with your application
// Any files that end with .css will be minified and bundled with your application even if you don't explicitly include them in your <head>
const _: Asset = asset!("/assets/tailwind.css");
The tailwind guide has more information on how to use tailwind with dioxus.

Conclusion
Dioxus provides first class support for assets, and makes it easy to include them in your application. You can include images, arbitrary files, and stylesheets in your application, and dioxus will automatically optimize them for production. This makes it easy to include assets in your application and ensure that they are optimized for production.

You can read more about assets and all the options available to optimize your assets in the manganis documentation.

User Input
Interfaces often need to provide a way to input data: e.g. text, numbers, checkboxes, etc. In Dioxus, there are two ways you can work with user input.

Controlled Inputs
With controlled inputs, you are directly in charge of the state of the input. This gives you a lot of flexibility, and makes it easy to keep things in sync. For example, this is how you would create a controlled text input:

src/input_controlled.rs

pub fn App() -> Element {
    let mut name = use_signal(|| "bob".to_string());

    rsx! {
        input {
            // we tell the component what to render
            value: "{name}",
            // and what to do when the value changes
            oninput: move |event| name.set(event.value())
        }
    }
}
bob
Notice the flexibility – you can:

Also display the same contents in another element, and they will be in sync
Transform the input every time it is modified (e.g. to make sure it is upper case)
Validate the input every time it changes
Have custom logic happening when the input changes (e.g. network request for autocompletion)
Programmatically change the value (e.g. a "randomize" button that fills the input with nonsense)
Uncontrolled Inputs
As an alternative to controlled inputs, you can simply let the platform keep track of the input values. If we don't tell a HTML input what content it should have, it will be editable anyway (this is built into the browser). This approach can be more performant, but less flexible. For example, it's harder to keep the input in sync with another element.

Since you don't necessarily have the current value of the uncontrolled input in state, you can access it either by listening to oninput events (similarly to controlled components), or, if the input is part of a form, you can access the form data in the form events (e.g. oninput or onsubmit):

src/input_uncontrolled.rs

pub fn App() -> Element {
    rsx! {
        form { onsubmit: move |event| { log::info!("Submitted! {event:?}") },
            input { name: "name" }
            input { name: "age" }
            input { name: "date" }
            input { r#type: "submit" }
        }
    }
}

Submitted! UiEvent { data: FormData { value: "", values: {"age": "very old", "date": "1966", "name": "Fred"} } }
Handling files
You can insert a file picker by using an input element of type file. This element supports the multiple attribute, to let you pick more files at the same time. You can select a folder by adding the directory attribute: Dioxus will map this attribute to browser specific attributes, because there is no standardized way to allow a directory to be selected.

type is a Rust keyword, so when specifying the type of the input field, you have to write it as r#type:"file".

Extracting the selected files is a bit different from what you may typically use in Javascript.

The FormData event contains a files field with data about the uploaded files. This field contains a FileEngine struct which lets you fetch the filenames selected by the user. This example saves the filenames of the selected files to a Vec:

src/input_fileengine.rs

pub fn App() -> Element {
    let mut filenames: Signal<Vec<String>> = use_signal(Vec::new);
    rsx! {
        input {
            // tell the input to pick a file
            r#type: "file",
            // list the accepted extensions
            accept: ".txt,.rs",
            // pick multiple files
            multiple: true,
            onchange: move |evt| {
                if let Some(file_engine) = &evt.files() {
                    let files = file_engine.files();
                    for file_name in files {
                        filenames.write().push(file_name);
                    }
                }
            }
        }
    }
}
If you're planning to read the file content, you need to do it asynchronously, to keep the rest of the UI interactive. This example event handler loads the content of the selected files in an async closure:

src/input_fileengine_async.rs

onchange: move |evt| {
    async move {
        if let Some(file_engine) = evt.files() {
            let files = file_engine.files();
            for file_name in &files {
                if let Some(file) = file_engine.read_file_to_string(file_name).await
                {
                    files_uploaded.write().push(file);
                }
            }
        }
    }
}
Lastly, this example shows you how to select a folder, by setting the directory attribute to true.

src/input_fileengine_folder.rs

input {
    r#type: "file",
    // Select a folder by setting the directory attribute
    directory: true,
    onchange: move |evt| {
        if let Some(file_engine) = evt.files() {
            let files = file_engine.files();
            for file_name in files {
                println!("{}", file_name);
            }
        }
    }
}

Coroutines
Another tool in your async toolbox are coroutines. Coroutines are futures that can have values sent to them.

Like regular futures, code in a coroutine will run until the next await point before yielding. This low-level control over asynchronous tasks is quite powerful, allowing for infinitely looping tasks like WebSocket polling, background timers, and other periodic actions.

use_coroutine
The use_coroutine hook allows you to create a coroutine. Most coroutines we write will be polling loops using await.

src/use_coroutine_reference.rs

use futures_util::StreamExt;

fn app() {
    let ws: Coroutine<()> = use_coroutine(|rx| async move {
        // Connect to some sort of service
        let mut conn = connect_to_ws_server().await;

        // Wait for data on the service
        while let Some(msg) = conn.next().await {
            // handle messages
        }
    });
}
For many services, a simple async loop will handle the majority of use cases.

Yielding Values
To yield values from a coroutine, simply bring in a Signal handle and set the value whenever your coroutine completes its work.

The future must be 'static – so any values captured by the task cannot carry any references to cx, such as a Signal.

You can use to_owned to create a clone of the hook handle which can be moved into the async closure.

src/use_coroutine_reference.rs

let mut sync_status = use_signal(|| Status::Launching);
let sync_task = use_coroutine(move |rx: UnboundedReceiver<SyncAction>| async move {
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        sync_status.set(Status::Working);
    }
});
To make this a bit less verbose, Dioxus exports the to_owned! macro which will create a binding as shown above, which can be quite helpful when dealing with many values.

src/use_coroutine_reference.rs

let sync_status = use_signal(|| Status::Launching);
let load_status = use_signal(|| Status::Launching);
let sync_task = use_coroutine(|rx: UnboundedReceiver<SyncAction>| {
    async move {
        // ...
    }
});
Sending Values
You might've noticed the use_coroutine closure takes an argument called rx. What is that? Well, a common pattern in complex apps is to handle a bunch of async code at once. With libraries like Redux Toolkit, managing multiple promises at once can be challenging and a common source of bugs.

With Coroutines, we can centralize our async logic. The rx parameter is an Channel that allows code external to the coroutine to send data into the coroutine. Instead of looping on an external service, we can loop on the channel itself, processing messages from within our app without needing to spawn a new future. To send data into the coroutine, we would call "send" on the handle.

src/use_coroutine_reference.rs

use futures_util::StreamExt;

enum ProfileUpdate {
    SetUsername(String),
    SetAge(i32),
}

let profile = use_coroutine(|mut rx: UnboundedReceiver<ProfileUpdate>| async move {
    let mut server = connect_to_server().await;

    while let Some(msg) = rx.next().await {
        match msg {
            ProfileUpdate::SetUsername(name) => server.update_username(name).await,
            ProfileUpdate::SetAge(age) => server.update_age(age).await,
        }
    }
});

rsx! {
    button { onclick: move |_| profile.send(ProfileUpdate::SetUsername("Bob".to_string())),
        "Update username"
    }
}
Note: In order to use/run the rx.next().await statement you will need to extend the [ Stream] trait (used by [ UnboundedReceiver] ) by adding 'futures_util' as a dependency to your project and adding the use futures_util::stream::StreamExt;.

For sufficiently complex apps, we could build a bunch of different useful "services" that loop on channels to update the app.

src/use_coroutine_reference.rs

let profile = use_coroutine(profile_service);
let editor = use_coroutine(editor_service);
let sync = use_coroutine(sync_service);

async fn profile_service(rx: UnboundedReceiver<ProfileCommand>) {
    // do stuff
}

async fn sync_service(rx: UnboundedReceiver<SyncCommand>) {
    // do stuff
}

async fn editor_service(rx: UnboundedReceiver<EditorCommand>) {
    // do stuff
}
We can combine coroutines with Global State to emulate Redux Toolkit's Thunk system with much less headache. This lets us store all of our app's state within a task and then simply update the "view" values stored in Atoms. It cannot be understated how powerful this technique is: we get all the perks of native Rust tasks with the optimizations and ergonomics of global state. This means your actual state does not need to be tied up in a system like Signal::global or Redux – the only Atoms that need to exist are those that are used to drive the display/UI.

src/use_coroutine_reference.rs

static USERNAME: GlobalSignal<String> = Signal::global(|| "default".to_string());

fn app() -> Element {
    use_coroutine(sync_service);

    rsx! {
        Banner {}
    }
}

fn Banner() -> Element {
    rsx! {
        h1 { "Welcome back, {USERNAME}" }
    }
}
Now, in our sync service, we can structure our state however we want. We only need to update the view values when ready.

src/use_coroutine_reference.rs

use futures_util::StreamExt;

static USERNAME: GlobalSignal<String> = Signal::global(|| "default".to_string());
static ERRORS: GlobalSignal<Vec<String>> = Signal::global(|| Vec::new());

enum SyncAction {
    SetUsername(String),
}

async fn sync_service(mut rx: UnboundedReceiver<SyncAction>) {
    while let Some(msg) = rx.next().await {
        match msg {
            SyncAction::SetUsername(name) => {
                if set_name_on_server(&name).await.is_ok() {
                    *USERNAME.write() = name;
                } else {
                    *ERRORS.write() = vec!["Failed to set username".to_string()];
                }
            }
        }
    }
}
Automatic injection into the Context API
Coroutine handles are automatically injected through the context API. You can use the use_coroutine_handle hook with the message type as a generic to fetch a handle.

src/use_coroutine_reference.rs

fn Child() -> Element {
    let sync_task = use_coroutine_handle::<SyncAction>();

    sync_task.send(SyncAction::SetUsername);

    todo!()
}

Describing the UI
Dioxus is a declarative framework. This means that instead of telling Dioxus what to do (e.g. to "create an element" or "set the color to red") we simply declare what we want the UI to look like using RSX.

You have already seen a simple example of RSX syntax in the "hello world" application:

src/hello_world_desktop.rs

// define a component that renders a div with the text "Hello, world!"
fn App() -> Element {
    rsx! {
        div { "Hello, world!" }
    }
}
Here, we use the rsx! macro to declare that we want a div element, containing the text "Hello, world!". Dioxus takes the RSX and constructs a UI from it.

RSX Features
RSX is very similar to HTML in that it describes elements with attributes and children. Here's an empty button element in RSX, as well as the resulting HTML:

src/rsx_overview.rs

rsx! {
    button {
        // attributes / listeners
        // children
        "Hello, World!"
    }
}
Hello, World!
Attributes
Attributes (and event handlers) modify the behavior or appearance of the element they are attached to. They are specified inside the {} brackets, using the name: value syntax. You can provide the value as a literal in the RSX:

src/rsx_overview.rs

rsx! {
    img {
        src: "https://avatars.githubusercontent.com/u/79236386?s=200&v=4",
        class: "primary_button",
        width: "10px",
    }
}

Some attributes, such as the type attribute for input elements won't work on their own in Rust. This is because type is a reserved Rust keyword. To get around this, Dioxus uses the r# specifier:

src/rsx_overview.rs

rsx! {
    input { r#type: "text", color: "red" }
}
Note: All attributes defined in dioxus-html follow the snake_case naming convention. They transform their snake_case names to HTML's camelCase attributes.

Note: Styles can be used directly outside of the style: attribute. In the above example, color: "red" is turned into style="color: red".

Conditional Attributes
You can also conditionally include attributes by using an if statement without an else branch. This is useful for adding an attribute only if a certain condition is met:

src/rsx_overview.rs

let large_font = true;
rsx! {
    div { class: if large_font { "text-xl" }, "Hello, World!" }
}
Hello, World!
Custom Attributes
Dioxus has a pre-configured set of attributes that you can use. RSX is validated at compile time to make sure you didn't specify an invalid attribute. If you want to override this behavior with a custom attribute name, specify the attribute in quotes:

src/rsx_overview.rs

rsx! {
    div { "style": "width: 20px; height: 20px; background-color: red;" }
}
Special Attributes
While most attributes are simply passed on to the HTML, some have special behaviors.

The HTML Escape Hatch
If you're working with pre-rendered assets, output from templates, or output from a JS library, then you might want to pass HTML directly instead of going through Dioxus. In these instances, reach for dangerous_inner_html.

For example, shipping a markdown-to-Dioxus converter might significantly bloat your final application size. Instead, you'll want to pre-render your markdown to HTML and then include the HTML directly in your output. We use this approach for the Dioxus homepage:

src/dangerous_inner_html.rs

// this should come from a trusted source
let contents = "live <b>dangerously</b>";

rsx! {
    div { dangerous_inner_html: "{contents}" }
}
live dangerously
Note! This attribute is called "dangerous_inner_html" because it is dangerous to pass it data you don't trust. If you're not careful, you can easily expose cross-site scripting (XSS) attacks to your users.

If you're handling untrusted input, make sure to sanitize your HTML before passing it into dangerous_inner_html – or just pass it to a Text Element to escape any HTML tags.

Boolean Attributes
Most attributes, when rendered, will be rendered exactly as the input you provided. However, some attributes are considered "boolean" attributes and just their presence determines whether they affect the output. For these attributes, a provided value of "false" will cause them to be removed from the target element.

So this RSX wouldn't actually render the hidden attribute:

src/boolean_attribute.rs

rsx! {
    div { hidden: false, "hello" }
}
hello
Not all attributes work like this however. Only the following attributes have this behavior:

allowfullscreen
allowpaymentrequest
async
autofocus
autoplay
checked
controls
default
defer
disabled
formnovalidate
hidden
ismap
itemscope
loop
multiple
muted
nomodule
novalidate
open
playsinline
readonly
required
reversed
selected
truespeed
For any other attributes, a value of "false" will be sent directly to the DOM.

Interpolation
Similarly to how you can format Rust strings, you can also interpolate in RSX text. Use {variable} to Display the value of a variable in a string, or {variable:?} to use the Debug representation:

src/rsx_overview.rs

let coordinates = (42, 0);
let country = "es";
rsx! {
    div {
        class: "country-{country}",
        left: "{coordinates.0:?}",
        top: "{coordinates.1:?}",
        // arbitrary expressions are allowed,
        // as long as they don't contain `{}`
        div { "{country.to_uppercase()}" }
        div { "{7*6}" }
        // {} can be escaped with {{}}
        div { "{{}}" }
    }
}
ES
42
{}
Children
To add children to an element, put them inside the {} brackets after all attributes and listeners in the element. They can be other elements, text, or components. For example, you could have an ol (ordered list) element, containing 3 li (list item) elements, each of which contains some text:

src/rsx_overview.rs

rsx! {
    ol {
        li { "First Item" }
        li { "Second Item" }
        li { "Third Item" }
    }
}
First Item
Second Item
Third Item
Fragments
You can render multiple elements at the top level of rsx! and they will be automatically grouped.

src/rsx_overview.rs

rsx! {
    p { "First Item" }
    p { "Second Item" }
}
First Item

Second Item

Expressions
You can include arbitrary Rust expressions as children within RSX by surrounding your expression with {}s. Any expression that implements IntoDynNode can be used within rsx. This is useful for displaying data from an iterator:

src/rsx_overview.rs

let text = "Dioxus";
rsx! {
    span {
        {text.to_uppercase()}
        // create a list of text from 0 to 9
        {(0..10).map(|i| rsx! {
        "{i}"
        })}
    }
}
DIOXUS0123456789
Loops
In addition to iterators you can also use for loops directly within RSX:

src/rsx_overview.rs

rsx! {
    // use a for loop where the body itself is RSX
    div {
        // create a list of text from 0 to 9
        for i in 0..3 {
            // NOTE: the body of the loop is RSX not a rust statement
            div { "{i}" }
        }
    }
    // iterator equivalent
    div {
        {(0..3).map(|i| rsx! {
            div { "{i}" }
        })}
    }
}
0
1
2
0
1
2
If statements
You can also use if statements without an else branch within RSX:

src/rsx_overview.rs

rsx! {
    // use if statements without an else
    if true {
        div { "true" }
    }
}
true

Components
Just like you wouldn't want to write a complex program in a single, long, main function, you shouldn't build a complex UI in a single App function. Instead, you should break down the functionality of an app in logical parts called components.

A component is a Rust function, named in UpperCamelCase, that either takes no parameters or a properties struct and returns an Element describing the UI it wants to render.

src/hello_world_desktop.rs

// define a component that renders a div with the text "Hello, world!"
fn App() -> Element {
    rsx! {
        div { "Hello, world!" }
    }
}
You'll probably want to add #![allow(non_snake_case)] to the top of your crate to avoid warnings about UpperCamelCase component names

A Component is responsible for some rendering task – typically, rendering an isolated part of the user interface. For example, you could have an About component that renders a short description of Dioxus Labs:

src/components.rs

pub fn About() -> Element {
    rsx! {
        p {
            b { "Dioxus Labs" }
            " An Open Source project dedicated to making Rust UI wonderful."
        }
    }
}
Dioxus Labs An Open Source project dedicated to making Rust UI wonderful.

Then, you can render your component in another component, similarly to how elements are rendered:

src/components.rs

pub fn App() -> Element {
    rsx! {
        About {}
        About {}
    }
}
Dioxus Labs An Open Source project dedicated to making Rust UI wonderful.

Dioxus Labs An Open Source project dedicated to making Rust UI wonderful.

At this point, it might seem like components are nothing more than functions. However, as you learn more about the features of Dioxus, you'll see that they are actually more powerful!

Component Props
Just like you can pass arguments to a function or attributes to an element, you can pass props to a component that customize its behavior! The components we've seen so far didn't accept any props – so let's write some components that do.

derive(Props)
Component props are a single struct annotated with #[derive(PartialEq, Clone, Props)]. For a component to accept props, the type of its argument must be YourPropsStruct.

Example:

src/component_owned_props.rs

#[derive(PartialEq, Props, Clone)]
struct LikesProps {
    score: i32,
}

fn Likes(props: LikesProps) -> Element {
    rsx! {
        div {
            "This post has "
            b { "{props.score}" }
            " likes"
        }
    }
}
You can then pass prop values to the component the same way you would pass attributes to an element:

src/component_owned_props.rs

pub fn App() -> Element {
    rsx! { Likes { score: 42 } }
}
This post has 42 likes
Prop Options
The #[derive(Props)] macro has some features that let you customize the behavior of props.

Optional Props
You can create optional fields by using the Option<…> type for a field:

src/component_props_options.rs

#[derive(PartialEq, Clone, Props)]
struct OptionalProps {
    title: String,
    subtitle: Option<String>,
}

fn Title(props: OptionalProps) -> Element {
    rsx! {
        h1 { "{props.title}: ", {props.subtitle.unwrap_or_else(|| "No subtitle provided".to_string())} }
    }
}
Then, you can choose to either provide them or not:

src/component_props_options.rs

Title { title: "Some Title" }
Title { title: "Some Title", subtitle: "Some Subtitle" }
// Providing an Option explicitly won't compile though:
// Title {
//     title: "Some Title",
//     subtitle: None,
// },
Explicitly Required Option
If you want to explicitly require an Option, and not an optional prop, you can annotate it with #[props(!optional)]:

src/component_props_options.rs

#[derive(PartialEq, Clone, Props)]
struct ExplicitOptionProps {
    title: String,
    #[props(!optional)]
    subtitle: Option<String>,
}

fn ExplicitOption(props: ExplicitOptionProps) -> Element {
    rsx! {
        h1 { "{props.title}: ", {props.subtitle.unwrap_or_else(|| "No subtitle provided".to_string())} }
    }
}
Then, you have to explicitly pass either Some("str") or None:

src/component_props_options.rs

ExplicitOption { title: "Some Title", subtitle: None }
ExplicitOption { title: "Some Title", subtitle: Some("Some Title".to_string()) }
// This won't compile:
// ExplicitOption {
//     title: "Some Title",
// },
Default Props
You can use #[props(default = 42)] to make a field optional and specify its default value:

src/component_props_options.rs

#[derive(PartialEq, Props, Clone)]
struct DefaultProps {
    // default to 42 when not provided
    #[props(default = 42)]
    number: i64,
}

fn DefaultComponent(props: DefaultProps) -> Element {
    rsx! { h1 { "{props.number}" } }
}
Then, similarly to optional props, you don't have to provide it:

src/component_props_options.rs

DefaultComponent { number: 5 }
DefaultComponent {}
Automatic Conversion with into
It is common for Rust functions to accept impl Into<SomeType> rather than just SomeType to support a wider range of parameters. If you want similar functionality with props, you can use #[props(into)]. For example, you could add it on a String prop – and &str will also be automatically accepted, as it can be converted into String:

src/component_props_options.rs

#[derive(PartialEq, Props, Clone)]
struct IntoProps {
    #[props(into)]
    string: String,
}

fn IntoComponent(props: IntoProps) -> Element {
    rsx! { h1 { "{props.string}" } }
}
Then, you can use it so:

src/component_props_options.rs

IntoComponent { string: "some &str" }
The component macro
So far, every Component function we've seen had a corresponding ComponentProps struct to pass in props. This was quite verbose... Wouldn't it be nice to have props as simple function arguments? Then we wouldn't need to define a Props struct, and instead of typing props.whatever, we could just use whatever directly!

component allows you to do just that. Instead of typing the "full" version:


#[derive(Props, Clone, PartialEq)]
struct TitleCardProps {
    title: String,
}

fn TitleCard(props: TitleCardProps) -> Element {
    rsx!{
        h1 { "{props.title}" }
    }
}
...you can define a function that accepts props as arguments. Then, just annotate it with #[component], and the macro will turn it into a regular Component for you:


#[component]
fn TitleCard(title: String) -> Element {
    rsx!{
        h1 { "{title}" }
    }
}
While the new Component is shorter and easier to read, this macro should not be used by library authors since you have less control over Prop documentation.

Component Children
In some cases, you may wish to create a component that acts as a container for some other content, without the component needing to know what that content is. To achieve this, create a prop of type Element:

src/component_element_props.rs

#[derive(PartialEq, Clone, Props)]
struct ClickableProps {
    href: String,
    body: Element,
}

fn Clickable(props: ClickableProps) -> Element {
    rsx! {
        a { href: "{props.href}", class: "fancy-button", {props.body} }
    }
}
Then, when rendering the component, you can pass in the output of rsx!{...}:

src/component_element_props.rs

rsx! {
    Clickable {
        href: "https://www.youtube.com/watch?v=C-M2hs3sXGo",
        body: rsx! {
            "How to " i { "not" } " be seen"
        }
    }
}
Warning: While it may compile, do not include the same Element more than once in the RSX. The resulting behavior is unspecified.

The children field
Rather than passing the RSX through a regular prop, you may wish to accept children similarly to how elements can have children. The "magic" children prop lets you achieve this:

src/component_children.rs

#[derive(PartialEq, Clone, Props)]
struct ClickableProps {
    href: String,
    children: Element,
}

fn Clickable(props: ClickableProps) -> Element {
    rsx! {
        a { href: "{props.href}", class: "fancy-button", {props.children} }
    }
}
This makes using the component much simpler: simply put the RSX inside the {} brackets – and there is no need for a render call or another macro!

src/component_children.rs

rsx! {
    Clickable { href: "https://www.youtube.com/watch?v=C-M2hs3sXGo",
        "How to "
        i { "not" }
        " be seen"
    }
}
How to not be seenEvent Handlers
Event handlers are used to respond to user actions. For example, an event handler could be triggered when the user clicks, scrolls, moves the mouse, or types a character.

Event handlers are attached to elements. For example, we usually don't care about all the clicks that happen within an app, only those on a particular button.

Event handlers are similar to regular attributes, but their name usually starts with on- and they accept closures as values. The closure will be called whenever the event it listens for is triggered and will be passed that event.

For example, to handle clicks on an element, we can specify an onclick handler:

src/event_click.rs

rsx! {
    button { onclick: move |event| log::info!("Clicked! Event: {event:?}"), "click me!" }
}
click me!
The Event object
Event handlers receive an Event object containing information about the event. Different types of events contain different types of data. For example, mouse-related events contain MouseData, which tells you things like where the mouse was clicked and what mouse buttons were used.

In the example above, this event data was logged to the terminal:


Clicked! Event: UiEvent { bubble_state: Cell { value: true }, data: MouseData { coordinates: Coordinates { screen: (242.0, 256.0), client: (26.0, 17.0), element: (16.0, 7.0), page: (26.0, 17.0) }, modifiers: (empty), held_buttons: EnumSet(), trigger_button: Some(Primary) } }
Clicked! Event: UiEvent { bubble_state: Cell { value: true }, data: MouseData { coordinates: Coordinates { screen: (242.0, 256.0), client: (26.0, 17.0), element: (16.0, 7.0), page: (26.0, 17.0) }, modifiers: (empty), held_buttons: EnumSet(), trigger_button: Some(Primary) } }
To learn what the different event types for HTML provide, read the events module docs.

Event propagation
Some events will trigger first on the element the event originated at upward. For example, a click event on a button inside a div would first trigger the button's event listener and then the div's event listener.

For more information about event propagation see the mdn docs on event bubbling

If you want to prevent this behavior, you can call stop_propagation() on the event:

src/event_nested.rs

rsx! {
    div { onclick: move |_event| {},
        "outer"
        button {
            onclick: move |event| {
                event.stop_propagation();
            },
            "inner"
        }
    }
}
Prevent Default
Some events have a default behavior. For keyboard events, this might be entering the typed character. For mouse events, this might be selecting some text.

In some instances, might want to avoid this default behavior. For this, you can add the prevent_default attribute with the name of the handler whose default behavior you want to stop. This attribute can be used for multiple handlers using their name separated by spaces:

src/event_prevent_default.rs

rsx! {
    a {
        href: "https://example.com",
        onclick: |evt| {
            evt.prevent_default();
            log::info!("link clicked")
        },
        "example.com"
    }
}
example.com
Any event handlers will still be called.

Handler Props
Sometimes, you might want to make a component that accepts an event handler. A simple example would be a FancyButton component, which accepts an onclick handler:

src/event_handler_prop.rs

#[derive(PartialEq, Clone, Props)]
pub struct FancyButtonProps {
    onclick: EventHandler<MouseEvent>,
}

pub fn FancyButton(props: FancyButtonProps) -> Element {
    rsx! {
        button {
            class: "fancy-button",
            onclick: move |evt| props.onclick.call(evt),
            "click me pls."
        }
    }
}
Then, you can use it like any other handler:

src/event_handler_prop.rs

rsx! {
    FancyButton {
        onclick: move |event| println!("Clicked! {event:?}"),
    }
}
Note: just like any other attribute, you can name the handlers anything you want! Any closure you pass in will automatically be turned into an EventHandler.

Custom Data
Event Handlers are generic over any type, so you can pass in any data you want to them, e.g:

src/event_handler_prop.rs

struct ComplexData(i32);

#[derive(PartialEq, Clone, Props)]
pub struct CustomFancyButtonProps {
    onclick: EventHandler<ComplexData>,
}

pub fn CustomFancyButton(props: CustomFancyButtonProps) -> Element {
    rsx! {
        button {
            class: "fancy-button",
            onclick: move |_| props.onclick.call(ComplexData(0)),
            "click me pls."
        }
    }
}
Returning a value from an event handler
If you want to accept a closure like an event handler that returns a value, you can use the Callback type. The callback type accepts two generic arguments, I, the input type, and O, the output type. Just like EventHandler, Callback is automatically converted in props and can be easily copied into anywhere in your component:

src/event_handler_prop.rs

#[derive(PartialEq, Clone, Props)]
pub struct CounterProps {
    modify: Callback<u32, u32>,
}

pub fn Counter(props: CounterProps) -> Element {
    let mut count = use_signal(|| 1);

    rsx! {
        button {
            onclick: move |_| count.set(props.modify.call(count())),
            "double"
        }
        div { "count: {count}" }
    }
}