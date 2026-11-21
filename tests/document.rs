fn create_document() {
    let doc1 = Document::from("This is the body text of the document");

    let doc2 = Document::new();
    doc2.push("This is the body text of the document");

    let doc3 = Document::new();
    doc3.push_span("This is", |span| span);
    doc3.push_span(" the body text of the document", |span| span);

    assert_eq!(doc1.text(), doc2.text());
    assert_eq!(doc1.text(), doc3.text());
}

fn create_document_with_attributes() {
    let Document = Document::from("Document with Attribute").with_attr("author", "me");
}
