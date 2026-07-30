from io import BytesIO
from pathlib import Path

from PIL import Image, ImageDraw
from reportlab.lib import colors
from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import getSampleStyleSheet
from reportlab.platypus import (
    Image as ReportLabImage,
    PageBreak,
    Paragraph,
    SimpleDocTemplate,
    Spacer,
    Table,
    TableStyle,
)


ROOT = Path(__file__).resolve().parent
OUTPUT = ROOT / "structured-retrieval-fixture.pdf"


def chart_image(accent: str, label: str) -> BytesIO:
    image = Image.new("RGB", (720, 360), "white")
    draw = ImageDraw.Draw(image)
    draw.rectangle((0, 0, 719, 359), outline="#20272d", width=4)
    draw.rectangle((55, 55, 665, 285), fill="#f2f4f5", outline="#80909a", width=2)
    bars = [90, 150, 115, 205, 170]
    for index, height in enumerate(bars):
        left = 95 + index * 105
        draw.rectangle((left, 270 - height, left + 54, 270), fill=accent)
    draw.text((60, 310), label, fill="#20272d")
    buffer = BytesIO()
    image.save(buffer, format="PNG")
    buffer.seek(0)
    return buffer


def build() -> None:
    styles = getSampleStyleSheet()
    document = SimpleDocTemplate(
        str(OUTPUT),
        pagesize=A4,
        rightMargin=48,
        leftMargin=48,
        topMargin=48,
        bottomMargin=48,
        title="Structured PDF Retrieval Fixture",
        author="SLICER verification",
    )
    story = [
        Paragraph("Structured PDF Retrieval Fixture", styles["Title"]),
        Spacer(1, 12),
        Paragraph(
            "This paragraph must be indexed directly from the PDF structure. "
            "The verification keyword is ORCHID-MODULE-ALPHA.",
            styles["BodyText"],
        ),
        Spacer(1, 16),
        Table(
            [
                ["Module", "Expected behavior"],
                ["Paragraph", "Indexed without a model request"],
                ["Image", "Analyzed only when visual enrichment is requested"],
                ["Location", "Returned with page number and normalized bbox"],
            ],
            colWidths=[130, 330],
            style=TableStyle(
                [
                    ("BACKGROUND", (0, 0), (-1, 0), colors.HexColor("#20272d")),
                    ("TEXTCOLOR", (0, 0), (-1, 0), colors.white),
                    ("GRID", (0, 0), (-1, -1), 0.75, colors.HexColor("#80909a")),
                    ("FONTNAME", (0, 0), (-1, 0), "Helvetica-Bold"),
                    ("VALIGN", (0, 0), (-1, -1), "TOP"),
                    ("PADDING", (0, 0), (-1, -1), 7),
                ]
            ),
        ),
        Spacer(1, 22),
        Paragraph("Two visual modules on the same page", styles["Heading2"]),
        Spacer(1, 8),
        Table(
            [
                [
                    ReportLabImage(chart_image("#cf4c3c", "CHART-RED-DELTA"), 225, 112.5),
                    ReportLabImage(chart_image("#247f70", "CHART-GREEN-SIGMA"), 225, 112.5),
                ],
                [
                    Paragraph("Figure 1. Red quarterly totals.", styles["BodyText"]),
                    Paragraph("Figure 2. Green quarterly totals.", styles["BodyText"]),
                ],
            ],
            colWidths=[235, 235],
            style=TableStyle(
                [
                    ("VALIGN", (0, 0), (-1, -1), "TOP"),
                    ("LEFTPADDING", (0, 0), (-1, -1), 4),
                    ("RIGHTPADDING", (0, 0), (-1, -1), 4),
                    ("TOPPADDING", (0, 0), (-1, -1), 4),
                    ("BOTTOMPADDING", (0, 0), (-1, -1), 4),
                ]
            ),
        ),
        PageBreak(),
        Paragraph("Second page fallback reference", styles["Title"]),
        Spacer(1, 14),
        Paragraph(
            "This second page provides a separate searchable module. "
            "The verification keyword is COBALT-PAGE-BETA.",
            styles["BodyText"],
        ),
    ]
    document.build(story)


if __name__ == "__main__":
    build()
    print(OUTPUT)
