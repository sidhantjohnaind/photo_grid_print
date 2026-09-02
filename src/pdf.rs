use std::io::Write;

pub struct PdfPage {
    pub jpeg_data: Vec<u8>,
    pub width_px: u32,
    pub height_px: u32,
}

pub fn create_pdf(pages: &[PdfPage], page_w_pt: f64, page_h_pt: f64) -> Vec<u8> {
    let mut buffer = Vec::new();
    let num_pages = pages.len();

    // PDF 1.4 Header
    buffer.extend_from_slice(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n");

    let mut offsets = Vec::new();
    offsets.push(0);

    let catalog_id = 1;
    let pages_id = 2;

    // Catalog
    offsets.push(buffer.len());
    let _ = writeln!(&mut buffer, "{} 0 obj\n<< /Type /Catalog /Pages {} 0 R >>\nendobj", catalog_id, pages_id);

    // Pages
    let mut kids = String::new();
    for i in 0..num_pages {
        let page_obj_id = 3 + i * 3;
        kids.push_str(&format!("{} 0 R ", page_obj_id));
    }
    offsets.push(buffer.len());
    let _ = writeln!(
        &mut buffer,
        "{} 0 obj\n<< /Type /Pages /Kids [{}] /Count {} >>\nendobj",
        pages_id, kids.trim(), num_pages
    );

    // Each Page
    for (i, page) in pages.iter().enumerate() {
        let page_obj_id = 3 + i * 3;
        let contents_obj_id = 4 + i * 3;
        let image_obj_id = 5 + i * 3;

        let content_stream = format!(
            "q {:.4} 0 0 {:.4} 0 0 cm /Im{} Do Q",
            page_w_pt, page_h_pt, i + 1
        );

        // Page Object
        offsets.push(buffer.len());
        let _ = writeln!(
            &mut buffer,
            "{} 0 obj\n<< /Type /Page /Parent {} 0 R /MediaBox [0 0 {:.4} {:.4}] /Resources << /XObject << /Im{} {} 0 R >> >> /Contents {} 0 R >>\nendobj",
            page_obj_id, pages_id, page_w_pt, page_h_pt, i + 1, image_obj_id, contents_obj_id
        );

        // Contents Object
        offsets.push(buffer.len());
        let _ = writeln!(
            &mut buffer,
            "{} 0 obj\n<< /Length {} >>\nstream\n{}\nendstream\nendobj",
            contents_obj_id,
            content_stream.len(),
            content_stream
        );

        // Image XObject
        offsets.push(buffer.len());
        let img_header = format!(
            "{} 0 obj\n<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
            image_obj_id, page.width_px, page.height_px, page.jpeg_data.len()
        );
        buffer.extend_from_slice(img_header.as_bytes());
        buffer.extend_from_slice(&page.jpeg_data);
        buffer.extend_from_slice(b"\nendstream\nendobj\n");
    }

    // XRef Table
    let xref_offset = buffer.len();
    let total_objs = 2 + num_pages * 3;
    let _ = writeln!(&mut buffer, "xref\n0 {}", total_objs + 1);
    let _ = writeln!(&mut buffer, "0000000000 65535 f ");
    for offset in &offsets[1..] {
        let _ = writeln!(&mut buffer, "{:010} 00000 n ", offset);
    }

    // Trailer
    let _ = writeln!(
        &mut buffer,
        "trailer\n<< /Size {} /Root {} 0 R >>\nstartxref\n{}\n%%EOF",
        total_objs + 1,
        catalog_id,
        xref_offset
    );

    buffer
}
