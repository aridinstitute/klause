// This test uses the example of a Bible passage
fn find() {
    let doc = Doc::new(); // or whatever setup

    assert!(doc.find().get_passage(), doc.text()); // empty find is equivalent to doc.text();

    doc.find()
        .within("verse")
        .
}
