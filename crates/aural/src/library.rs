//! The account's library as the UI shows it: liked songs, saved playlists and albums, and the
//! tracks of any one of them. Thin conversions over `ytmusic`, which does the requests.

use std::time::Duration;

use anyhow::Result;
use ytmusic::YtMusic;

/// The side covers are asked for in lists and grids, in pixels.
pub const THUMB_EDGE: u32 = 226;
/// The side of the big cover on a detail page and in the player.
pub const COVER_EDGE: u32 = 544;

/// A playable song.
#[derive(Clone, Debug, PartialEq)]
pub struct Song {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration: Option<Duration>,
    /// The cover url without a size; ask for one with [`sized`].
    pub cover: Option<String>,
}

/// A playlist or an album, as a card in the library.
#[derive(Clone, Debug, PartialEq)]
pub struct Collection {
    pub id: String,
    pub kind: Kind,
    pub title: String,
    pub subtitle: String,
    pub cover: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Playlist,
    Album,
    Artist,
}

/// The library's first page: everything the library screen lists.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Library {
    pub liked: Vec<Song>,
    pub playlists: Vec<Collection>,
    pub albums: Vec<Collection>,
}

/// Loads liked songs, playlists and albums together. A part that fails stays empty.
pub async fn load(api: &YtMusic) -> Result<Library> {
    let (liked, playlists, albums) = tokio::join!(
        api.liked_songs(),
        api.library_playlists(),
        api.library_albums()
    );
    if let (Err(error), Err(_), Err(_)) = (&liked, &playlists, &albums) {
        anyhow::bail!("cannot load the library: {error:#}");
    }
    Ok(Library {
        liked: liked
            .unwrap_or_default()
            .into_iter()
            .filter_map(song)
            .collect(),
        playlists: playlists
            .unwrap_or_default()
            .into_iter()
            .map(|playlist| Collection {
                id: playlist.id,
                kind: Kind::Playlist,
                subtitle: match playlist.track_count {
                    Some(count) => songs_count(count as usize),
                    None => playlist.author.unwrap_or_default(),
                },
                title: playlist.title,
                cover: cover(&playlist.thumbnails),
            })
            .collect(),
        albums: albums
            .unwrap_or_default()
            .into_iter()
            .map(|album| Collection {
                id: album.browse_id,
                kind: Kind::Album,
                subtitle: album
                    .artists
                    .first()
                    .map(|artist| artist.name.clone())
                    .unwrap_or_default(),
                title: album.title,
                cover: cover(&album.thumbnails),
            })
            .collect(),
    })
}

/// What a search found, in Sonora's three columns plus its best match.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Results {
    pub best: Option<Best>,
    pub songs: Vec<Song>,
    pub artists: Vec<Collection>,
    pub albums: Vec<Collection>,
    pub playlists: Vec<Collection>,
}

/// The result shown big above the columns.
#[derive(Clone, Debug, PartialEq)]
pub enum Best {
    Artist(Collection),
    Song(Song),
}

/// Searches songs, albums and playlists for `query` at once. A part that fails stays empty.
pub async fn search(api: &YtMusic, query: &str) -> Result<Results> {
    let (songs, albums, playlists, artists) = tokio::join!(
        api.search_songs(query),
        api.search_albums(query),
        api.search_playlists(query),
        search_artists(api, query)
    );
    if let (Err(error), Err(_), Err(_)) = (&songs, &albums, &playlists) {
        anyhow::bail!("cannot search: {error:#}");
    }
    let artists = artists
        .inspect_err(|error| log::warn!("search: no artists: {error:#}"))
        .unwrap_or_default();
    let albums = albums
        .unwrap_or_default()
        .into_iter()
        .map(|album| Collection {
            id: album.browse_id,
            kind: Kind::Album,
            subtitle: album
                .artists
                .first()
                .map(|artist| artist.name.clone())
                .unwrap_or_default(),
            title: album.title,
            cover: cover(&album.thumbnails),
        });
    let playlists = playlists
        .unwrap_or_default()
        .into_iter()
        .map(|playlist| Collection {
            id: playlist.id,
            kind: Kind::Playlist,
            subtitle: playlist.author.unwrap_or_default(),
            title: playlist.title,
            cover: cover(&playlist.thumbnails),
        });
    let songs: Vec<Song> = songs
        .unwrap_or_default()
        .into_iter()
        .filter_map(song)
        .collect();
    // The artist is the best match when the search names them; otherwise the top song is.
    let wanted = query.trim().to_lowercase();
    let best = artists
        .first()
        .filter(|artist| artist.title.to_lowercase() == wanted)
        .cloned()
        .map(Best::Artist)
        .or_else(|| songs.first().cloned().map(Best::Song));
    Ok(Results {
        best,
        songs,
        artists,
        albums: albums.collect(),
        playlists: playlists.collect(),
    })
}

/// Searches artists, which `ytmusic` has a filter for but no call: the same search request
/// with the artists filter, each result read off its list item.
async fn search_artists(api: &YtMusic, query: &str) -> Result<Vec<Collection>> {
    use ytmusic::nav::Nav as _;

    let response = api
        .execute(
            "search",
            ytmusic::Client::Music,
            serde_json::json!({ "query": query, "params": ytmusic::search::ARTISTS }),
        )
        .await?;
    let mut artists = Vec::new();
    for shelf in ytmusic::parse::find_renderers(&response, "musicShelfRenderer") {
        for item in shelf.items(&["contents"]) {
            let Some(renderer) = item.at(&["musicResponsiveListItemRenderer"]) else {
                continue;
            };
            let Some(id) = renderer
                .str_at(&["navigationEndpoint", "browseEndpoint", "browseId"])
                .filter(|id| id.starts_with("UC"))
            else {
                continue;
            };
            let columns = renderer.items(&["flexColumns"]);
            let text = |index: usize| {
                columns.get(index).and_then(|column| {
                    column.run_text(&["musicResponsiveListItemFlexColumnRenderer", "text"])
                })
            };
            let Some(name) = text(0) else {
                continue;
            };
            artists.push(Collection {
                id: id.to_owned(),
                kind: Kind::Artist,
                title: name,
                // YouTube's line starts with the kind ("Artist • 321M monthly audience").
                subtitle: text(1)
                    .map(|line| match line.split_once(" • ") {
                        Some((_, rest)) => audience(rest),
                        None => String::new(),
                    })
                    .unwrap_or_default(),
                cover: cover(&ytmusic::parse::thumbnails(renderer)),
            });
        }
    }
    Ok(artists)
}

/// The songs of a playlist or an album.
pub async fn tracks(api: &YtMusic, collection: &Collection) -> Result<Vec<Song>> {
    let tracks = match collection.kind {
        Kind::Playlist => api.playlist(&collection.id).await?.tracks,
        Kind::Artist => api.artist(&collection.id).await?.top_tracks,
        Kind::Album => {
            let detail = api.album(&collection.id).await?;
            let art = cover(&detail.album.thumbnails);
            return Ok(detail
                .tracks
                .into_iter()
                .filter_map(song)
                .map(|mut song| {
                    // Album rows carry video stills, not the album art.
                    song.cover = art.clone().or(song.cover);
                    song.album = song.album.or_else(|| Some(detail.album.title.clone()));
                    song
                })
                .collect());
        }
    };
    Ok(tracks.into_iter().filter_map(song).collect())
}

/// A track that can be played: it needs a video id.
pub fn song(track: ytmusic::Track) -> Option<Song> {
    Some(Song {
        id: track.video_id?,
        artist: track
            .artists
            .iter()
            .map(|artist| artist.name.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        album: track.album.map(|album| album.name),
        duration: track.duration,
        cover: cover(&track.thumbnails),
        title: track.title,
    })
}

/// The largest thumbnail's url.
fn cover(thumbnails: &[ytmusic::Thumbnail]) -> Option<String> {
    ytmusic::best_thumbnail(thumbnails).map(|thumb| thumb.url.clone())
}

/// `url` asked at `edge` pixels: Google's image host takes the size after the last `=`.
pub fn sized(url: &str, edge: u32) -> String {
    match url.rsplit_once('=') {
        Some((base, _)) if url.contains("googleusercontent.com") || url.contains("ggpht.com") => {
            format!("{base}=w{edge}-h{edge}-l90-rj")
        }
        _ => url.to_owned(),
    }
}

/// "1 canción", "12 canciones".
pub fn songs_count(count: usize) -> String {
    match count {
        1 => "1 canción".to_owned(),
        count => format!("{count} canciones"),
    }
}

/// `2:38` for a duration.
pub fn clock(duration: Duration) -> String {
    let seconds = duration.as_secs();
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// An artist's page, YouTube Music's: the banner and name, the top songs, then shelves of
/// albums, singles, playlists and related artists in the order the page lists them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ArtistPage {
    pub name: String,
    pub banner: Option<String>,
    /// "321 M oyentes mensuales", or the subscriber count.
    pub audience: Option<String>,
    pub description: Option<String>,
    pub top: Vec<Song>,
    pub shelves: Vec<Shelf>,
}

/// One titled row of an artist's page.
#[derive(Clone, Debug, PartialEq)]
pub struct Shelf {
    pub title: String,
    pub items: Vec<Collection>,
}

/// Loads an artist's page in one request, read with `ytmusic`'s parsers.
pub async fn artist_page(api: &YtMusic, id: &str) -> Result<ArtistPage> {
    use anyhow::Context as _;
    use ytmusic::nav::Nav as _;
    use ytmusic::parse;

    let response = api
        .execute(
            "browse",
            ytmusic::Client::Music,
            serde_json::json!({ "browseId": id }),
        )
        .await?;
    let header = parse::find_renderer(&response, "musicImmersiveHeaderRenderer")
        .or_else(|| parse::find_renderer(&response, "musicVisualHeaderRenderer"))
        .or_else(|| parse::find_renderer(&response, "musicHeaderRenderer"))
        .context("the artist page has no header")?;
    let audience = header
        .run_text(&["monthlyListenerCount"])
        .or_else(|| {
            header.run_text(&[
                "subscriptionButton",
                "subscribeButtonRenderer",
                "subscriberCountText",
            ])
        })
        .map(|line| audience(&line));
    let description = header
        .run_text(&["description"])
        .or_else(|| {
            parse::find_renderer(&response, "musicDescriptionShelfRenderer")
                .and_then(|shelf| shelf.run_text(&["description"]))
        })
        .filter(|text| !text.trim().is_empty());

    let top: Vec<Song> = parse::find_renderer(&response, "musicShelfRenderer")
        .map(|shelf| {
            shelf
                .items(&["contents"])
                .iter()
                .filter_map(parse::list_item_track)
                .filter_map(song)
                .collect()
        })
        .unwrap_or_default();

    let mut shelves = Vec::new();
    for carousel in parse::find_renderers(&response, "musicCarouselShelfRenderer") {
        let title = carousel
            .run_text(&["header", "musicCarouselShelfBasicHeaderRenderer", "title"])
            .unwrap_or_default();
        let items: Vec<Collection> = carousel
            .items(&["contents"])
            .iter()
            .filter_map(shelf_item)
            .collect();
        if !items.is_empty() {
            shelves.push(Shelf {
                title: shelf_title(&title),
                items,
            });
        }
    }

    log::info!(
        "artist: {} has {} top songs and {} shelves",
        header.run_text(&["title"]).unwrap_or_default(),
        top.len(),
        shelves.len()
    );
    Ok(ArtistPage {
        name: header.run_text(&["title"]).unwrap_or_default(),
        banner: cover(&parse::thumbnails(header)),
        audience,
        description,
        top,
        shelves,
    })
}

/// An album, playlist or artist of a carousel; videos and anything else are left out.
fn shelf_item(item: &serde_json::Value) -> Option<Collection> {
    use ytmusic::nav::Nav as _;
    use ytmusic::parse;

    if let Some(album) = parse::two_row_album(item) {
        let kind = match album.kind {
            ytmusic::AlbumKind::Single => "Sencillo",
            ytmusic::AlbumKind::Ep => "EP",
            _ => "Álbum",
        };
        let subtitle = match album.year {
            Some(year) => format!("{kind} • {year}"),
            None => kind.to_owned(),
        };
        return Some(Collection {
            id: album.browse_id,
            kind: Kind::Album,
            subtitle,
            title: album.title,
            cover: cover(&album.thumbnails),
        });
    }
    if let Some(playlist) = parse::two_row_playlist(item) {
        return Some(Collection {
            id: playlist.id,
            kind: Kind::Playlist,
            subtitle: playlist.author.unwrap_or_default(),
            title: playlist.title,
            cover: cover(&playlist.thumbnails),
        });
    }
    let renderer = item.at(&["musicTwoRowItemRenderer"])?;
    let id = renderer
        .str_at(&["navigationEndpoint", "browseEndpoint", "browseId"])
        .filter(|id| id.starts_with("UC"))?;
    Some(Collection {
        id: id.to_owned(),
        kind: Kind::Artist,
        title: renderer.run_text(&["title"])?,
        subtitle: renderer.run_text(&["subtitle"]).unwrap_or_default(),
        cover: cover(&parse::thumbnails(renderer)),
    })
}

/// The shelf titles YouTube Music sends in English, in Spanish.
/// YouTube's audience line in Spanish ("82.7M monthly audience" → "82.7M oyentes mensuales").
fn audience(line: &str) -> String {
    let words = [
        (" monthly audience", " oyentes mensuales"),
        (" monthly listeners", " oyentes mensuales"),
        (" subscribers", " suscriptores"),
    ];
    for (english, spanish) in words {
        if let Some(count) = line.strip_suffix(english) {
            return format!("{count}{spanish}");
        }
    }
    line.to_owned()
}

fn shelf_title(title: &str) -> String {
    match title {
        "Albums" => "Álbumes".into(),
        "Singles" | "Singles & EPs" | "Singles and EPs" => "Sencillos y EP".into(),
        "Featured on" => "Aparece en".into(),
        "Fans might also like" => "A los fans también podría gustarles".into(),
        "Videos" => "Videos".into(),
        "Live performances" => "Presentaciones en vivo".into(),
        "From your library" => "De tu biblioteca".into(),
        other => match other.strip_prefix("Playlists by ") {
            Some(artist) => format!("{artist} hizo estas playlists"),
            None => other.to_owned(),
        },
    }
}
