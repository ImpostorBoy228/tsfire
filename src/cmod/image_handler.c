#define STBI_NO_HDR
#define STBI_NO_PSD
#define STBI_NO_TGA
#define STBI_NO_PNM

#define STB_IMAGE_IMPLEMENTATION

#include "stb_image.h"

int idecode(const unsigned char *data, unsigned long len,
                    unsigned char **out_rgba, int *out_w, int *out_h) {
    int channels = 0;
    unsigned char *pixels = stbi_load_from_memory(
        data,
        (int)len,
        out_w,
        out_h,
        &channels,
        4              // нам нужен RGBA
    );

    if (!pixels) {
        return -1; // ошибка декодинга
    }

    *out_rgba = pixels;
    return 0; // критический успех
}

// free
void ifree(unsigned char *pixels) {
    if (pixels) {
        stbi_image_free(pixels);
    }
}
