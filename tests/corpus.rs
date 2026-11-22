use klause::{
    Corpus, CorpusError, Distance, Document, Error, Span, SpanId, Value, all_spans, spans_in,
};

fn document(id: &str, language: &str, word: &str) -> Document {
    let mut doc = Document::new(id)
        .with_attr("language", language)
        .with_layer("lines")
        .with_layer("words");
    let line = doc.push(word).unwrap();
    doc.add_span("lines", Span::from(line.clone()).with_id(1_u64))
        .unwrap();
    doc.add_span(
        "words",
        Span::from(line).with_id(1_u64).with_attr("pos", "noun"),
    )
    .unwrap();
    doc
}

#[test]
fn cross_doc_query_preserves_document_local_ranges_and_span_identity() {
    let mut corpus = Corpus::new();
    corpus.add_document(document("B", "en", "CAFÉ")).unwrap();
    corpus.add_document(document("C", "en", "tea")).unwrap();
    corpus.add_document(document("A", "fr", "Café")).unwrap();

    let query = || corpus.find().with_text_folded("cafe").in_layer("lines");
    let selections = query().get_selection().unwrap();
    assert_eq!(
        selections.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        ["A", "B"]
    );
    for (_, selection) in selections {
        assert_eq!(selection.ranges(), core::slice::from_ref(&(0..5)));
    }

    let passages = query().with_layers(["lines"]).get_passages().unwrap();
    assert_eq!(
        passages
            .iter()
            .map(|(id, p)| (*id, p.text()))
            .collect::<Vec<_>>(),
        [("A", "Café"), ("B", "CAFÉ"),]
    );
    assert!(
        passages
            .iter()
            .all(|(_, p)| p.spans_in("words").count() == 0)
    );

    let spans = query().get_spans(all_spans()).unwrap();
    assert_eq!(
        spans
            .iter()
            .map(|hit| (hit.document_id, hit.layer))
            .collect::<Vec<_>>(),
        [
            ("A", "lines"),
            ("A", "words"),
            ("B", "lines"),
            ("B", "words")
        ],
    );
    assert!(
        spans
            .iter()
            .all(|hit| hit.span.id() == Some(&SpanId::from(1_u64)))
    );
    assert_eq!(query().count().unwrap(), 2);
    assert_eq!(
        query()
            .count_spans(spans_in("words").with_attr("pos", "noun"))
            .unwrap(),
        2
    );
}

#[test]
fn text_and_proximity_cannot_cross_document_boundaries() {
    let mut corpus = Corpus::new();
    corpus.add_document(document("A", "en", "first")).unwrap();
    corpus.add_document(document("B", "en", "second")).unwrap();
    corpus
        .add_document(document("C", "en", "first second"))
        .unwrap();

    assert_eq!(
        corpus
            .find()
            .in_documents(["A", "B"])
            .with_text("first second")
            .count()
            .unwrap(),
        0
    );
    let phrase = corpus
        .find()
        .with_text("first second")
        .get_selection()
        .unwrap();
    assert_eq!(phrase.len(), 1);
    assert_eq!(phrase[0].0, "C");

    let second = spans_in("words").with_id(2_u64);
    let mut b = corpus.remove_document("B").unwrap();
    b.add_span("words", Span::from(0..6).with_id(2_u64))
        .unwrap();
    corpus.add_document(b).unwrap();
    assert_eq!(
        corpus
            .find()
            .in_documents(["A", "B"])
            .with_text("first")
            .followed_by(second, Distance::bytes(100))
            .count()
            .unwrap(),
        0
    );
}

#[test]
fn metadata_filters_run_before_document_queries_and_ids_are_deduplicated() {
    let mut corpus = Corpus::new();
    corpus.add_document(document("A", "fr", "bonjour")).unwrap();
    corpus.add_document(document("B", "en", "hello")).unwrap();
    corpus
        .add_document(Document::new("C").with_attr("language", ["en", "fr"]))
        .unwrap();

    let fr = Value::from("fr");
    let matches = corpus
        .find()
        .in_documents(["C", "B", "A", "A"])
        .in_span("lines", 1_u64)
        .filter_documents(|doc| doc.values("language").any(|value| value == &fr))
        .filter_documents(|doc| doc.layer("lines").is_some())
        .get_passages()
        .unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].0, "A");
    assert_eq!(matches[0].1.text(), "bonjour");
}

#[test]
fn value_counts_merge_across_documents_without_counting_a_span_twice() {
    let mut corpus = Corpus::new();
    for id in ["B", "A"] {
        let mut doc = Document::new(id).with_layer("tags");
        let phrase = doc.push("x x").unwrap();
        let span = Span::from(phrase)
            .with_attr("feature", ["common", "common", id])
            .with_attr(
                "feature",
                if id == "A" {
                    Value::Int(2)
                } else {
                    Value::Float(2.0)
                },
            );
        doc.add_span("tags", span).unwrap();
        doc.add_span("tags", Span::from(1..2).with_attr("feature", "outside"))
            .unwrap();
        corpus.add_document(doc).unwrap();
    }

    let query = || corpus.find().with_text("x");
    assert_eq!(query().count().unwrap(), 4);
    assert_eq!(query().count_spans("tags").unwrap(), 2);
    assert_eq!(
        query().count_values("tags", "feature").unwrap(),
        [
            (Value::Int(2), 2),
            (Value::from("common"), 2),
            (Value::from("A"), 1),
            (Value::from("B"), 1),
        ]
    );
}

#[test]
fn reports_unknown_and_duplicate_documents_and_qualifies_query_errors() {
    let mut corpus = Corpus::new();
    assert_eq!(corpus.find().count().unwrap(), 0);
    assert!(
        corpus
            .find()
            .count_values("words", "pos")
            .unwrap()
            .is_empty()
    );
    corpus.add_document(document("A", "en", "hello")).unwrap();
    corpus.add_document(Document::new("B")).unwrap();
    assert!(matches!(
        corpus.add_document(Document::new("A")),
        Err(CorpusError::DuplicateDocument(id)) if id == "A",
    ));
    assert_eq!(corpus.document("A").unwrap().text(), "hello");
    assert!(corpus.document("missing").is_none());
    assert!(matches!(
        corpus.find().in_documents(["A", "missing"]).filter_documents(|_| false).count(),
        Err(CorpusError::UnknownDocument(id)) if id == "missing",
    ));
    assert!(matches!(
        corpus.find().with_text("absent").count_spans("words"),
        Err(CorpusError::Query { document, error: Error::UnknownLayer(layer) })
            if document == "B" && layer == "words",
    ));
    assert_eq!(corpus.find().count().unwrap(), 2); // The empty document has a 0..0 hit.
    assert_eq!(
        corpus.find().in_documents([] as [&str; 0]).count().unwrap(),
        0
    );
}

#[test]
fn source_counts_use_each_documents_source_and_report_document_errors() {
    use klause::{Layer, SourceError, SpanFilter, SpanSource};
    use std::{io, ops::Range};

    struct Highlights(bool);

    impl SpanSource for Highlights {
        fn get_spans(&self, _: Range<u32>) -> Result<Vec<Span>, SourceError> {
            panic!("count_values should use the source's filter path")
        }

        fn find_spans(&self, _: &SpanFilter) -> Option<Result<Vec<Span>, SourceError>> {
            Some(if self.0 {
                Err(SourceError::new(io::Error::other("offline")))
            } else {
                Ok(vec![
                    Span::from(0..1).with_id(1_u64).with_attr("color", "gold"),
                ])
            })
        }
    }

    let mut corpus = Corpus::new();
    for (id, offline) in [("A", false), ("B", true), ("C", false)] {
        let mut doc =
            Document::new(id).with_layer(Layer::source("highlights", Highlights(offline)));
        doc.push("x").unwrap();
        corpus.add_document(doc).unwrap();
    }

    assert_eq!(
        corpus
            .find()
            .in_documents(["A", "C"])
            .count_values("highlights", "color")
            .unwrap(),
        [(Value::from("gold"), 2)],
    );
    assert!(matches!(
        corpus.find().count_values("highlights", "color"),
        Err(CorpusError::Query { document, error: Error::Source { layer, .. } })
            if document == "B" && layer == "highlights",
    ));
}
