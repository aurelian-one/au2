use std::rc::Rc;

use lipsum::{lipsum_title_with_rng, lipsum_words_with_rng};
use rand::Rng;

use au::item::Item;

pub fn mock_items(mut rng: impl Rng, n: usize) -> Vec<Item> {
    let item_id_gen = au::id::IdGen::default();
    let mut output: Vec<Item> = Vec::new();

    let mut item_ids: Vec<Rc<str>> = Vec::new();

    for _ in 0..n {
        let mut new_item = Item::default();
        new_item.id = Rc::from(item_id_gen.gen(&mut rng));
        new_item.rank = rng.gen();

        if rng.gen_ratio(3, 4) {
            // three quarters of the items will be text
            new_item.content_type = Rc::from("text/plain");

            let mut data = String::new();
            // half of the text items will have a useful title
            if rng.gen_ratio(1, 2) {
                data.push_str(lipsum_title_with_rng(&mut rng).as_str());
                data.push('\n');
                if rng.gen_ratio(1, 2) {
                    data.push('\n');
                }
            }

            for p_n in 0..rng.gen_range(1..5) {
                if p_n > 0 {
                    data.push('\n');
                    if rng.gen_ratio(1, 2) {
                        data.push('\n');
                    }
                }
                let words = rng.gen_range(10..40);
                data.push_str(lipsum_words_with_rng(&mut rng, words).as_str());
            }
            new_item.content = Rc::from(data.as_bytes())
        } else {
            // the remaining group are binary up to 1KB
            new_item.content_type = Rc::from("application/x-octet-stream");
            let data_len = rng.gen_range(1..1000);
            let mut data: Vec<u8> = Vec::with_capacity(data_len);
            rng.fill_bytes(data.as_mut_slice());
            new_item.content = Rc::from(data.as_slice())
        }

        // half the items will have class data attached
        if rng.gen_ratio(1, 2) {
            new_item.class = Some(Rc::from(lipsum_words_with_rng(&mut rng, 1)))
        }

        // now work out whether to attach to a parent
        if item_ids.len() > 0 && rng.gen_bool(1.0 / (item_ids.len() as f64).sqrt()) {
            let p: usize = rng.gen_range(0..item_ids.len());
            new_item.parent = Some(item_ids.get(p).unwrap().clone());
        }

        // push the id into the list
        item_ids.push(new_item.id.clone());
        output.push(new_item);
    }

    output
}



#[cfg(test)]
mod tests {
    use automerge::AutoCommit;

    use au::item::Project;

    use crate::mock_items;

    #[test]
    fn test_gen() {
        let mtng = &mut rand::thread_rng();

        let mut doc = AutoCommit::new();
        let mut p = Project::default();

        for x in mock_items(mtng, 100) {
            p.with_item(&x, &mut doc).unwrap();
        }
        
        assert!(p.list_children(None).len() > 0)
    }

}
