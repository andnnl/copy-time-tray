use crate::snippet::model::Snippet;
use gtk::prelude::*;

#[cfg(target_os = "linux")]
pub(super) fn populate_snippet_tree(
    store: &gtk::TreeStore,
    all_folders: &[&Snippet],
    all_snippets: &[&Snippet],
    query: &Option<String>,
    matching_ids: &std::collections::HashSet<String>,
    folders_to_show: &std::collections::HashSet<String>,
) {
    use gtk::prelude::TreeStoreExtManual;
    use std::collections::HashMap;

    let mut folders_by_parent: HashMap<Option<String>, Vec<usize>> = HashMap::new();
    for (i, folder) in all_folders.iter().enumerate() {
        folders_by_parent
            .entry(folder.parent_id.clone())
            .or_default()
            .push(i);
    }

    let mut snippets_by_parent: HashMap<Option<String>, Vec<usize>> = HashMap::new();
    for (i, s) in all_snippets.iter().enumerate() {
        snippets_by_parent
            .entry(s.parent_id.clone())
            .or_default()
            .push(i);
    }

    fn build_children(
        store: &gtk::TreeStore,
        all_folders: &[&Snippet],
        all_snippets: &[&Snippet],
        folders_by_parent: &HashMap<Option<String>, Vec<usize>>,
        snippets_by_parent: &HashMap<Option<String>, Vec<usize>>,
        parent_key: &Option<String>,
        parent_iter: Option<&gtk::TreeIter>,
        query: &Option<String>,
        matching_ids: &std::collections::HashSet<String>,
        folders_to_show: &std::collections::HashSet<String>,
    ) {
        if let Some(indices) = folders_by_parent.get(parent_key) {
            for &idx in indices {
                let folder = &all_folders[idx];
                if !folders_to_show.contains(&folder.id) {
                    continue;
                }
                let folder_iter = store.insert_with_values(
                    parent_iter,
                    None,
                    &[
                        (0, &folder.id),
                        (1, &folder.name),
                        (2, &""),
                        (3, &true),
                        (4, &"folder-symbolic"),
                    ],
                );
                build_children(
                    store,
                    all_folders,
                    all_snippets,
                    folders_by_parent,
                    snippets_by_parent,
                    &Some(folder.id.clone()),
                    Some(&folder_iter),
                    query,
                    matching_ids,
                    folders_to_show,
                );
                if let Some(snip_indices) = snippets_by_parent.get(&Some(folder.id.clone())) {
                    for &si in snip_indices {
                        let s = &all_snippets[si];
                        let should_show = if query.is_some() {
                            matching_ids.contains(&s.id)
                        } else {
                            true
                        };
                        if should_show {
                            store.insert_with_values(
                                Some(&folder_iter),
                                None,
                                &[
                                    (0, &s.id),
                                    (1, &s.name),
                                    (2, &s.content.clone().unwrap_or_default()),
                                    (3, &false),
                                    (4, &"text-x-script-symbolic"),
                                ],
                            );
                        }
                    }
                }
            }
        }

        if parent_key.is_none() {
            if let Some(snip_indices) = snippets_by_parent.get(&None) {
                for &si in snip_indices {
                    let s = &all_snippets[si];
                    let should_show = if query.is_some() {
                        matching_ids.contains(&s.id)
                    } else {
                        true
                    };
                    if should_show {
                        store.insert_with_values(
                            None,
                            None,
                            &[
                                (0, &s.id),
                                (1, &s.name),
                                (2, &s.content.clone().unwrap_or_default()),
                                (3, &false),
                                (4, &"text-x-script-symbolic"),
                            ],
                        );
                    }
                }
            }
        }
    }

    build_children(
        store,
        all_folders,
        all_snippets,
        &folders_by_parent,
        &snippets_by_parent,
        &None,
        None,
        query,
        matching_ids,
        folders_to_show,
    );
}
