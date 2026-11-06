export const prepareCharacterIcon = async (file: File): Promise<string> => {
  if (!file.type.startsWith("image/") || file.size > 10 * 1024 * 1024) {
    throw new Error("10 MB以下の画像を選択してください。");
  }
  try {
    const data = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => (typeof reader.result === "string" ? resolve(reader.result) : reject());
      reader.onerror = () => reject(reader.error);
      reader.readAsDataURL(file);
    });
    const image = new Image();
    image.src = data;
    await image.decode();
    const size = Math.min(image.naturalWidth, image.naturalHeight);
    if (!size) throw new Error();
    const canvas = document.createElement("canvas");
    canvas.width = 128;
    canvas.height = 128;
    const context = canvas.getContext("2d");
    if (!context) throw new Error();
    context.drawImage(
      image,
      (image.naturalWidth - size) / 2,
      (image.naturalHeight - size) / 2,
      size,
      size,
      0,
      0,
      128,
      128,
    );
    return canvas.toDataURL("image/png");
  } catch {
    throw new Error("画像を読み込めませんでした。");
  }
};
