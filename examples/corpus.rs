//! Query annotated text across documents and aggregate values.
use klause::{Corpus, Document, Span, Value, spans_in};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut corpus = Corpus::new();
    for (id, text) in [("fr", "Café"), ("en", "cafe")] {
        let mut doc = Document::new(id).with_layer("words");
        let word = doc.push(text)?;
        doc.add_span("words", Span::from(word).with_attr("pos", "noun"))?;
        corpus.add_document(doc)?;
    }

    let matches = corpus.find().with_text_folded("cafe");
    assert_eq!(
        matches.count_values("words", "pos")?,
        [(Value::from("noun"), 2)]
    );
    let passages = corpus
        .find()
        .within(spans_in("words").with_attr("pos", "noun"))
        .get_passages()?;
    assert_eq!(passages[0].0, "en");
    assert_eq!(passages[1].1.text(), "Café");
    Ok(())
}
