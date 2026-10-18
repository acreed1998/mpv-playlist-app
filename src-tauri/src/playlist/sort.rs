use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::time::SystemTime;

use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;

use super::model::{Direction, Playlist, SortKey, Track};

// Seeds are kept to 53 bits so they survive a trip through JavaScript,
// whose numbers can't represent every 64-bit integer exactly.
const MAX_SEED: u64 = (1 << 53) - 1;

pub fn new_seed() -> u64 {
    (uuid::Uuid::new_v4().as_u128() as u64) & MAX_SEED
}

fn dir_of(path: &str) -> &str {
    Path::new(path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("")
}

fn name_of(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}

// Flip a comparison result when the user wants descending order.
fn directed(ordering: Ordering, direction: Direction) -> Ordering {
    match direction {
        Direction::Ascending => ordering,
        Direction::Descending => ordering.reverse(),
    }
}

// Folder, then filename, then full path. Always ascending.
fn tie_break(a: &Track, b: &Track) -> Ordering {
    natord::compare_ignore_case(dir_of(&a.path), dir_of(&b.path))
        .then_with(|| natord::compare_ignore_case(name_of(&a.path), name_of(&b.path)))
        .then_with(|| a.path.cmp(&b.path))
}

// Ask the filesystem for each file's date. Only date sorts need this.
// Files we can't read are simply left out of the map.
pub fn read_times(paths: &[String], key: SortKey) -> HashMap<String, SystemTime> {
    let mut times = HashMap::new();
    if key != SortKey::Modified {
        return times;
    }
    for path in paths {
        let Ok(meta) = fs::metadata(path) else {
            continue;
        };
        if let Ok(modified) = meta.modified() {
            times.insert(path.clone(), modified);
        }
    }
    times
}

pub fn sorted_order(
    tracks: &[Track],
    key: SortKey,
    direction: Direction,
    seed: u64,
    times: &HashMap<String, SystemTime>,
) -> Vec<String> {
    // Work with references so we never copy whole tracks while sorting.
    let mut items: Vec<&Track> = tracks.iter().collect();

    match key {
        // `tracks` is already in the order things were added.
        // (Custom isn't a sort the user can request; the command rejects it.)
        SortKey::SourceOrder | SortKey::Custom => {
            if direction == Direction::Descending {
                items.reverse();
            }
        }

        SortKey::Name => items.sort_by(|a, b| {
            let primary = natord::compare_ignore_case(name_of(&a.path), name_of(&b.path));
            directed(primary, direction).then_with(|| tie_break(a, b))
        }),

        SortKey::Directory => items.sort_by(|a, b| {
            let primary = natord::compare_ignore_case(dir_of(&a.path), dir_of(&b.path));
            directed(primary, direction).then_with(|| tie_break(a, b))
        }),

        SortKey::Modified => items.sort_by(|a, b| {
            let primary = match (times.get(&a.path), times.get(&b.path)) {
                (Some(x), Some(y)) => directed(x.cmp(y), direction),
                (Some(_), None) => Ordering::Less, // known dates come first
                (None, Some(_)) => Ordering::Greater, // unknown dates come last
                (None, None) => Ordering::Equal,
            };
            primary.then_with(|| tie_break(a, b))
        }),

        SortKey::Random => {
            let mut rng = StdRng::seed_from_u64(seed);
            items.shuffle(&mut rng);
        }

        SortKey::ShuffleFolders => {
            // Group by folder. A BTreeMap iterates in key order, so the folder
            // order is the same every time before we shuffle it.
            let mut groups: BTreeMap<&str, Vec<&Track>> = BTreeMap::new();
            for track in items {
                groups.entry(dir_of(&track.path)).or_default().push(track);
            }

            let mut folders: Vec<Vec<&Track>> = groups.into_values().collect();
            for folder in &mut folders {
                folder.sort_by(|a, b| tie_break(a, b)); // order inside stays natural
            }

            let mut rng = StdRng::seed_from_u64(seed);
            folders.shuffle(&mut rng);

            items = folders.into_iter().flatten().collect();
        }

        // Length has its own function (length_order), which needs durations.
        SortKey::Length => {}
    }

    items.into_iter().map(|track| track.id.clone()).collect()
}

pub fn keeps_sorted_on_add(key: SortKey) -> bool {
    matches!(
        key,
        SortKey::SourceOrder
            | SortKey::Name
            | SortKey::Modified
            | SortKey::Directory
            | SortKey::Length
    )
}

pub fn reapply_current(
    playlist: &mut Playlist,
    times: &HashMap<String, SystemTime>,
    durations: &HashMap<String, f64>,
) {
    let key = playlist.sort.key;
    if !keeps_sorted_on_add(key) {
        return;
    }
    let order = if key == SortKey::Length {
        length_order(&playlist.tracks, playlist.sort.direction, durations)
    } else {
        // The seed only matters for random sorts, which never reach this line.
        sorted_order(&playlist.tracks, key, playlist.sort.direction, 0, times)
    };
    playlist.order = order;
}

// Order by video length. Lengths are compared in whole seconds, so near-equal
// files fall through to the usual tie-break chain. Unknown lengths sort last
// in both directions.
pub fn length_order(
    tracks: &[Track],
    direction: Direction,
    durations: &HashMap<String, f64>,
) -> Vec<String> {
    let mut items: Vec<&Track> = tracks.iter().collect();

    items.sort_by(|a, b| {
        let primary = match (durations.get(&a.path), durations.get(&b.path)) {
            (Some(x), Some(y)) => {
                // Lengths are never negative, so the cast to u64 is exact.
                directed((x.round() as u64).cmp(&(y.round() as u64)), direction)
            }
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        };
        primary.then_with(|| tie_break(a, b))
    });

    items.into_iter().map(|track| track.id.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn tracks(paths: &[&str]) -> Vec<Track> {
        paths.iter().map(|p| Track::new(*p)).collect()
    }

    // Translate an order of IDs back into paths so assertions are readable.
    fn in_order(tracks: &[Track], order: &[String]) -> Vec<String> {
        order
            .iter()
            .map(|id| tracks.iter().find(|t| &t.id == id).unwrap().path.clone())
            .collect()
    }

    fn sort(t: &[Track], key: SortKey, dir: Direction) -> Vec<String> {
        in_order(t, &sorted_order(t, key, dir, 1, &HashMap::new()))
    }

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    #[test]
    fn name_sort_is_natural_and_ignores_case() {
        let t = tracks(&["/v/episode10.mp4", "/v/Episode2.mp4", "/v/episode1.mp4"]);
        assert_eq!(
            sort(&t, SortKey::Name, Direction::Ascending),
            vec!["/v/episode1.mp4", "/v/Episode2.mp4", "/v/episode10.mp4"]
        );
        assert_eq!(
            sort(&t, SortKey::Name, Direction::Descending),
            vec!["/v/episode10.mp4", "/v/Episode2.mp4", "/v/episode1.mp4"]
        );
    }

    #[test]
    fn directory_sort_breaks_ties_by_filename() {
        let t = tracks(&["/b/2.mp4", "/a/10.mp4", "/a/2.mp4", "/b/1.mp4"]);
        assert_eq!(
            sort(&t, SortKey::Directory, Direction::Ascending),
            vec!["/a/2.mp4", "/a/10.mp4", "/b/1.mp4", "/b/2.mp4"]
        );
    }

    #[test]
    fn date_sort_breaks_ties_by_directory_then_name() {
        let t = tracks(&["/b/x.mp4", "/a/y.mp4", "/a/w.mp4"]);
        let times = HashMap::from([
            ("/b/x.mp4".to_string(), at(100)),
            ("/a/y.mp4".to_string(), at(100)),
            ("/a/w.mp4".to_string(), at(50)),
        ]);

        let asc = sorted_order(&t, SortKey::Modified, Direction::Ascending, 1, &times);
        assert_eq!(in_order(&t, &asc), vec!["/a/w.mp4", "/a/y.mp4", "/b/x.mp4"]);

        // Descending flips the dates, but the tied pair stays in A->Z order.
        let desc = sorted_order(&t, SortKey::Modified, Direction::Descending, 1, &times);
        assert_eq!(
            in_order(&t, &desc),
            vec!["/a/y.mp4", "/b/x.mp4", "/a/w.mp4"]
        );
    }

    #[test]
    fn unknown_dates_sort_last_in_both_directions() {
        let t = tracks(&["/a/old.mp4", "/c/unknown.mp4", "/a/new.mp4"]);
        let times = HashMap::from([
            ("/a/old.mp4".to_string(), at(10)),
            ("/a/new.mp4".to_string(), at(20)),
        ]);

        for dir in [Direction::Ascending, Direction::Descending] {
            let order = sorted_order(&t, SortKey::Modified, dir, 1, &times);
            assert_eq!(in_order(&t, &order).last().unwrap(), "/c/unknown.mp4");
        }
    }

    #[test]
    fn source_order_can_be_reversed() {
        let t = tracks(&["/z.mp4", "/a.mp4", "/m.mp4"]);
        assert_eq!(
            sort(&t, SortKey::SourceOrder, Direction::Ascending),
            vec!["/z.mp4", "/a.mp4", "/m.mp4"]
        );
        assert_eq!(
            sort(&t, SortKey::SourceOrder, Direction::Descending),
            vec!["/m.mp4", "/a.mp4", "/z.mp4"]
        );
    }

    #[test]
    fn random_is_repeatable_for_a_seed_and_keeps_every_track() {
        let names: Vec<String> = (0..20).map(|i| format!("/v/{i}.mp4")).collect();
        let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        let t = tracks(&refs);
        let none = HashMap::new();

        let a = sorted_order(&t, SortKey::Random, Direction::Ascending, 1, &none);
        let b = sorted_order(&t, SortKey::Random, Direction::Ascending, 1, &none);
        let c = sorted_order(&t, SortKey::Random, Direction::Ascending, 2, &none);

        assert_eq!(a, b); // same seed, same result
        assert_ne!(a, c); // different seed, (almost certainly) different result

        let mut sorted_ids = a.clone();
        sorted_ids.sort();
        let mut expected: Vec<String> = t.iter().map(|x| x.id.clone()).collect();
        expected.sort();
        assert_eq!(sorted_ids, expected); // a shuffle must not lose or invent tracks
    }

    #[test]
    fn shuffle_folders_keeps_each_folder_together_and_in_order() {
        let t = tracks(&[
            "/c/2.mp4", "/a/2.mp4", "/b/1.mp4", "/a/1.mp4", "/c/1.mp4", "/b/2.mp4",
        ]);

        for seed in 0..20 {
            let order = sorted_order(
                &t,
                SortKey::ShuffleFolders,
                Direction::Ascending,
                seed,
                &HashMap::new(),
            );
            let result = in_order(&t, &order);
            assert_eq!(result.len(), 6);

            for pair in result.chunks(2) {
                assert_eq!(dir_of(&pair[0]), dir_of(&pair[1])); // same folder, side by side
                assert!(name_of(&pair[0]) < name_of(&pair[1])); // 1.mp4 before 2.mp4
            }
        }
    }

    #[test]
    fn adding_to_a_name_sorted_playlist_slots_files_in_place() {
        let mut p = Playlist::new("t", "Test");
        p.add_files(vec!["/v/a.mp4".to_string(), "/v/c.mp4".to_string()]);
        p.sort.key = SortKey::Name;

        p.add_files(vec!["/v/b.mp4".to_string()]);
        reapply_current(&mut p, &HashMap::new(), &HashMap::new());

        assert_eq!(
            in_order(&p.tracks, &p.order),
            vec!["/v/a.mp4", "/v/b.mp4", "/v/c.mp4"]
        );
    }

    #[test]
    fn adding_to_a_random_playlist_appends_without_reshuffling() {
        let mut p = Playlist::new("t", "Test");
        p.add_files(vec!["/v/z.mp4".to_string(), "/v/m.mp4".to_string()]);
        p.sort.key = SortKey::Random;
        let before = p.order.clone();

        p.add_files(vec!["/v/a.mp4".to_string()]);
        reapply_current(&mut p, &HashMap::new(), &HashMap::new());

        assert_eq!(&p.order[..2], &before[..]); // existing order untouched
        assert_eq!(in_order(&p.tracks, &p.order).last().unwrap(), "/v/a.mp4");
    }

    #[test]
    fn length_sort_uses_whole_seconds_and_ties_fall_back_to_folder_then_name() {
        let t = tracks(&["/b/x.mp4", "/a/y.mp4", "/a/w.mp4", "/c/long.mp4"]);
        let durations = HashMap::from([
            ("/b/x.mp4".to_string(), 60.2),
            ("/a/y.mp4".to_string(), 59.8), // both round to 60 seconds
            ("/a/w.mp4".to_string(), 30.0),
            ("/c/long.mp4".to_string(), 600.0),
        ]);

        let asc = length_order(&t, Direction::Ascending, &durations);
        assert_eq!(
            in_order(&t, &asc),
            vec!["/a/w.mp4", "/a/y.mp4", "/b/x.mp4", "/c/long.mp4"]
        );

        // Descending flips the lengths, but the tied pair stays folder A->Z.
        let desc = length_order(&t, Direction::Descending, &durations);
        assert_eq!(
            in_order(&t, &desc),
            vec!["/c/long.mp4", "/a/y.mp4", "/b/x.mp4", "/a/w.mp4"]
        );
    }

    #[test]
    fn unknown_lengths_sort_last_in_both_directions() {
        let t = tracks(&["/a/unknown.mp4", "/a/short.mp4", "/a/long.mp4"]);
        let durations = HashMap::from([
            ("/a/short.mp4".to_string(), 10.0),
            ("/a/long.mp4".to_string(), 99.0),
        ]);

        for dir in [Direction::Ascending, Direction::Descending] {
            let order = length_order(&t, dir, &durations);
            assert_eq!(in_order(&t, &order).last().unwrap(), "/a/unknown.mp4");
        }
    }

    #[test]
    fn adding_to_a_length_sorted_playlist_slots_files_by_duration() {
        let mut p = Playlist::new("t", "Test");
        p.add_files(vec!["/v/short.mp4".into(), "/v/long.mp4".into()]);
        p.sort.key = SortKey::Length;
        p.add_files(vec!["/v/mid.mp4".into()]);

        let durations = HashMap::from([
            ("/v/short.mp4".to_string(), 10.0),
            ("/v/mid.mp4".to_string(), 50.0),
            ("/v/long.mp4".to_string(), 90.0),
        ]);
        reapply_current(&mut p, &HashMap::new(), &durations);

        assert_eq!(
            in_order(&p.tracks, &p.order),
            vec!["/v/short.mp4", "/v/mid.mp4", "/v/long.mp4"]
        );
    }
}
