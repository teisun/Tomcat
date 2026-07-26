export {
  ALLOWED_ATTACHMENT_MIME_TYPES,
  ALLOWED_FILE_MIME_TYPES,
  ALLOWED_IMAGE_MIME_TYPES,
  IMAGE_MAX_BYTES,
  PDF_MAX_BYTES,
  decodeBase64Strict,
  isSupportedAttachmentMime,
  safeAttachmentFilename,
  validateAttachmentCandidate as validateImageCandidate,
} from "./attachmentProtocol";
export type {
  AllowedAttachmentMimeType,
  AllowedFileMimeType,
  AllowedImageMimeType,
  AttachmentCandidate as ImageAttachmentCandidate,
  AttachmentFeedback as ImageAttachmentFeedback,
  AttachmentResultItem as ImageAttachmentResultItem,
  AttachFilesIntent as AttachImagesIntent,
  AttachFilesResult as AttachImagesResult,
} from "./attachmentProtocol";
