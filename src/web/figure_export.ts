import { field, input, paragraph, showDialog } from "./dialog";

type FigureFormat = "png" | "jpeg" | "pdf";

type FigureExportContext = {
  format: FigureFormat;
  svgUrl: () => string;
  filename: () => string;
  download: (blob: Blob, filename: string) => void;
  page: () => [number, number];
};

const MAX_EDGE = 8192;
const MAX_PIXELS = 64_000_000;

function rasterFilename(filename: string, format: FigureFormat): string {
  const base = filename.replace(/\.agr$/i, "") || "grace-figure";
  if (format === "pdf") return `${base}.pdf`;
  return `${base}.${format === "jpeg" ? "jpg" : "png"}`;
}

function loadImage(blob: Blob): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(blob);
    const image = new Image();
    image.addEventListener(
      "load",
      () => {
        URL.revokeObjectURL(url);
        resolve(image);
      },
      { once: true },
    );
    image.addEventListener(
      "error",
      () => {
        URL.revokeObjectURL(url);
        reject(new Error("Grace could not render the SVG for download."));
      },
      { once: true },
    );
    image.src = url;
  });
}

async function rasterize(
  svgUrl: string,
  page: [number, number],
  format: FigureFormat,
  width: number,
  quality: number,
): Promise<Blob> {
  if (!Number.isInteger(width) || width < 1 || width > MAX_EDGE) {
    throw new Error(`Enter a width between 1 and ${MAX_EDGE} pixels.`);
  }
  if (!Number.isFinite(quality) || quality < 1 || quality > 100) {
    throw new Error("JPEG quality must be from 1 through 100.");
  }
  if (!svgUrl) throw new Error("Grace has not drawn a figure yet.");

  const [pageWidth, pageHeight] = page;
  if (!(pageWidth > 0) || !(pageHeight > 0)) {
    throw new Error("Grace has an invalid page size.");
  }
  const height = Math.round((width * pageHeight) / pageWidth);
  if (height < 1 || height > MAX_EDGE || width * height > MAX_PIXELS) {
    throw new Error("Choose a smaller raster size.");
  }

  const response = await fetch(svgUrl);
  if (!response.ok) throw new Error("Grace could not read the SVG for download.");
  const image = await loadImage(await response.blob());
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d");
  if (!context) throw new Error("Your browser cannot create an image export.");
  if (format === "jpeg") {
    context.fillStyle = "#ffffff";
    context.fillRect(0, 0, width, height);
  }
  context.drawImage(image, 0, 0, width, height);

  const type = format === "jpeg" ? "image/jpeg" : "image/png";
  return new Promise((resolve, reject) => {
    canvas.toBlob(
      (blob) => {
        if (blob) resolve(blob);
        else reject(new Error("Your browser could not encode this image."));
      },
      type,
      format === "jpeg" ? quality / 100 : undefined,
    );
  });
}

async function vectorPdf(svgUrl: string, page: [number, number]): Promise<Blob> {
  if (!svgUrl) throw new Error("Grace has not drawn a figure yet.");
  const [width, height] = page;
  if (!(width > 0) || !(height > 0)) throw new Error("Grace has an invalid page size.");

  const response = await fetch(svgUrl);
  if (!response.ok) throw new Error("Grace could not read the SVG for download.");
  const svgDocument = new DOMParser().parseFromString(await response.text(), "image/svg+xml");
  if (svgDocument.querySelector("parsererror")) {
    throw new Error("Grace could not read the SVG for vector PDF export.");
  }
  const svg = svgDocument.documentElement;
  if (svg.namespaceURI !== "http://www.w3.org/2000/svg" || svg.localName !== "svg") {
    throw new Error("Grace did not produce an SVG figure for vector PDF export.");
  }

  const holder = window.document.createElement("div");
  holder.style.cssText = "position:fixed;left:-10000px;top:-10000px;visibility:hidden";
  holder.append(svg);
  window.document.body.append(holder);
  try {
    const [{ jsPDF }, { svg2pdf }] = await Promise.all([import("jspdf"), import("svg2pdf.js")]);
    const pdf = new jsPDF({
      orientation: width > height ? "landscape" : "portrait",
      unit: "pt",
      format: [width, height],
      compress: true,
      putOnlyUsedFonts: true,
      precision: 16,
    });
    await svg2pdf(svg, pdf, { x: 0, y: 0, width, height, loadExternalStyleSheets: false });
    return pdf.output("blob");
  } finally {
    holder.remove();
  }
}

export function figureExportDialog(context: FigureExportContext): void {
  const defaultWidth = Math.min(MAX_EDGE, Math.max(1, Math.round(context.page()[0] * 2)));
  const isJpeg = context.format === "jpeg";
  const isPdf = context.format === "pdf";
  showDialog(
    isPdf ? "Export vector PDF" : isJpeg ? "Export JPEG" : "Export PNG",
    [
      paragraph(
        isPdf
          ? "PDF keeps Grace's lines, text, and shapes as vector geometry."
          : isJpeg
            ? "JPEG exports use a white background."
            : "PNG exports preserve transparency.",
      ),
      ...(!isPdf ? [field("Width (pixels)", input("width", String(defaultWidth), "number"))] : []),
      ...(isJpeg ? [field("Quality", input("quality", "92", "number"))] : []),
    ],
    async (data) => {
      if (isPdf) {
        context.download(
          await vectorPdf(context.svgUrl(), context.page()),
          rasterFilename(context.filename(), "pdf"),
        );
        return;
      }
      const width = Number(data.get("width"));
      const quality = isJpeg ? Number(data.get("quality")) : 100;
      const blob = await rasterize(
        context.svgUrl(),
        context.page(),
        context.format,
        width,
        quality,
      );
      context.download(blob, rasterFilename(context.filename(), context.format));
    },
  );
}
