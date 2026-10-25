export interface SerializedBody {
  body: ArrayBuffer;
  contentType: string;
}

/**
 * Encodes a FormData as bytes plus its multipart Content-Type (with the boundary).
 * WebKitGTK crashes when a custom-scheme request body holds a File or Blob, so uploads send bytes instead.
 */
export const serializeFormData = async (form: FormData): Promise<SerializedBody> => {
  const encoded = new Response(form);
  return { body: await encoded.arrayBuffer(), contentType: encoded.headers.get('Content-Type') ?? '' };
};
