export const SUPPORTED_EXTENSIONS = [
  ".pdf",
  ".doc",
  ".docx",
  ".ppt",
  ".pptx",
] as const;

export function getFileExtension(path: string): string {
  const lastDot = path.lastIndexOf(".");
  if (lastDot === -1) return "";
  return path.slice(lastDot).toLowerCase();
}

export function isSupportedFileType(path: string): boolean {
  const ext = getFileExtension(path);
  return (SUPPORTED_EXTENSIONS as readonly string[]).includes(ext);
}

export function isOfficeFileType(path: string): boolean {
  const ext = getFileExtension(path);
  return [".doc", ".docx", ".ppt", ".pptx"].includes(ext);
}

export function getUnsupportedReason(path: string): string | null {
  if (isSupportedFileType(path)) return null;
  const ext = getFileExtension(path) || "(无扩展名)";
  return `不支持的文件类型: ${ext}，当前支持 PDF、DOC、DOCX、PPT、PPTX`;
}
