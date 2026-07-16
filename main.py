import cv2
import os
import mido
import argparse
import sys


def preprocess_frame(frame, resolution):
    resized = cv2.resize(frame, resolution, cv2.INTER_LINEAR)
    gray = cv2.cvtColor(resized, cv2.COLOR_BGR2GRAY)
    _, binary = cv2.threshold(gray, 127, 255, cv2.THRESH_BINARY | cv2.THRESH_OTSU)
    return binary


def convert_pixel_values(image):
    image_list = image.tolist()
    return [[1 if pixel == 0 else 0 for pixel in row] for row in image_list]


def calculate_sysex_data(binary_data):
    sysex_list = []

    for col_start in [0, 5, 10]:
        for row in range(16):
            value = (
                16 * binary_data[row][col_start + 0]
                + 8 * binary_data[row][col_start + 1]
                + 4 * binary_data[row][col_start + 2]
                + 2 * binary_data[row][col_start + 3]
                + 1 * binary_data[row][col_start + 4]
            )
            sysex_list.append(value)

    for row in range(16):
        sysex_list.append(16 * binary_data[row][15])

    return sysex_list


def calculate_sysex_data_8850(binary_data):
    sysex_list = []

    for section in range(16):
        sysex_list.append([])
        for row in range(section * 4, section * 4 + 4):
            for col_start in range(0, 156, 6):
                value = (
                    32 * binary_data[row][col_start + 0]
                    + 16 * binary_data[row][col_start + 1]
                    + 8 * binary_data[row][col_start + 2]
                    + 4 * binary_data[row][col_start + 3]
                    + 2 * binary_data[row][col_start + 4]
                    + 1 * binary_data[row][col_start + 5]
                )
                sysex_list[section].append(value)
            col_start = 156
            value = (
                32 * binary_data[row][col_start + 0]
                + 16 * binary_data[row][col_start + 1]
                + 8 * binary_data[row][col_start + 2]
                + 4 * binary_data[row][col_start + 3]
            )
            sysex_list[section].append(value)

    return sysex_list


def calculate_sysex_data_sd90(binary_data):
    sysex_list = []

    for section in range(16):
        sysex_list.append([])
        for row in range(section * 4, section * 4 + 4):
            for col_start in range(0, 126, 6):
                value = (
                    32 * binary_data[row][col_start + 0]
                    + 16 * binary_data[row][col_start + 1]
                    + 8 * binary_data[row][col_start + 2]
                    + 4 * binary_data[row][col_start + 3]
                    + 2 * binary_data[row][col_start + 4]
                    + 1 * binary_data[row][col_start + 5]
                )
                sysex_list[section].append(value)
            col_start = 126
            value = (
                32 * binary_data[row][col_start + 0]
                + 16 * binary_data[row][col_start + 1]
            )
            sysex_list[section].append(value)
            sysex_list[section] += [0] * 5

    return sysex_list


def create_sysex_messages(sysex_data, video_fps, frameskip, midi_frame_count):
    messages = []
    sysex_header = [0x41, 0x10, 0x45, 0x12]
    sysex_address = [0x10, 0x01, 0x00]

    checksum = 128 - (sum(sysex_address) + sum(sysex_data)) % 128
    if checksum == 128:
        checksum = 0

    image_sysex = sysex_header + sysex_address + sysex_data + [checksum]

    frame_time = midi_frame_count * (frameskip + 1) / video_fps
    frame_next_time = (midi_frame_count + 1) * (frameskip + 1) / video_fps
    frame_tick = int(frame_time * 960)
    frame_next_tick = int(frame_next_time * 960)

    frame_length_tick = frame_next_tick - frame_tick

    messages.append(
        mido.Message(
            "sysex",
            data=image_sysex,
            time=frame_length_tick,
        )
    )

    return messages


def create_sysex_messages_8850(
    sysex_data, video_fps, frameskip, midi_frame_count, interlace
):
    messages = []
    sysex_header = [0x41, 0x10, 0x45, 0x12]

    if interlace:
        if midi_frame_count % 2 == 0:
            section_order = [0, 2, 4, 6, 8, 10, 12, 14, 15]
        else:
            section_order = [1, 3, 5, 7, 9, 11, 13, 15]
    else:
        section_order = range(16)

    for section in section_order:
        sysex_address = [0x20, section, 0x00]
        section_data = sysex_data[section]

        checksum = 128 - (sum(sysex_address) + sum(section_data)) % 128
        if checksum == 128:
            checksum = 0

        image_sysex = sysex_header + sysex_address + section_data + [checksum]

        frame_time = midi_frame_count * (frameskip + 1) / video_fps
        section_time = (
            section_order.index(section) / len(section_order) / (video_fps / (frameskip + 1)) + frame_time
        )
        section_next_time = (section_order.index(section) + 1) / len(section_order) / (
            video_fps / (frameskip + 1)
        ) + frame_time
        section_tick = int(section_time * 960)
        section_next_tick = int(section_next_time * 960)

        section_length_tick = section_next_tick - section_tick

        messages.append(
            mido.Message(
                "sysex",
                data=image_sysex,
                time=section_length_tick,
            )
        )

    return messages


def process(videofile, frameskip, sc8850, interlace, sd90):
    if not os.path.exists(videofile):
        print(f"Error: File '{videofile}' does not exist.")
        sys.exit(1)

    video = cv2.VideoCapture(videofile)
    video_fps = video.get(cv2.CAP_PROP_FPS)

    midi = mido.MidiFile()
    midi_track_meta = mido.MidiTrack()
    midi_track_data = mido.MidiTrack()

    midi_track_meta.append(mido.MetaMessage("set_tempo", tempo=500000))

    video_frame_count = 0
    midi_frame_count = 0
    while True:
        ret, frame_image = video.read()
        if not ret:
            break

        if video_frame_count % (frameskip + 1) == 0:
            if sc8850:
                if sd90:
                    frame_image_processed = preprocess_frame(frame_image, (128, 64))
                    frame_image_binarylist = convert_pixel_values(frame_image_processed)
                    sysex_data = calculate_sysex_data_sd90(frame_image_binarylist)
                else:
                    frame_image_processed = preprocess_frame(frame_image, (160, 64))
                    frame_image_binarylist = convert_pixel_values(frame_image_processed)
                    sysex_data = calculate_sysex_data_8850(frame_image_binarylist)
                sysex_messages = create_sysex_messages_8850(
                    sysex_data, video_fps, frameskip, midi_frame_count, interlace
                )
            else:
                frame_image_processed = preprocess_frame(frame_image, (16, 16))
                frame_image_binarylist = convert_pixel_values(frame_image_processed)
                sysex_data = calculate_sysex_data(frame_image_binarylist)
                sysex_messages = create_sysex_messages(
                    sysex_data, video_fps, frameskip, midi_frame_count
                )
            for message in sysex_messages:
                midi_track_data.append(message)
            midi_frame_count += 1

        video_frame_count += 1

    video.release()

    midi.tracks.append(midi_track_meta)
    midi.tracks.append(midi_track_data)

    base_name = os.path.splitext(videofile)[0]
    suffix = ""
    if sc8850:
        suffix += "s"
    if sd90:
        suffix += "e"
    if interlace:
        suffix += "i"
    suffix += f"f{frameskip}"
    output_file = f"{base_name}_{suffix}.mid"
    midi.save(output_file)


def main():
    parser = argparse.ArgumentParser(
        description="Convert videos to MIDI files which can be read by Roland Sound Canvas."
    )
    parser.add_argument("input_video", type=str, help="Path to the input video file")
    parser.add_argument(
        "-f",
        "--frameskip",
        type=int,
        default=0,
        help="Number of frames to skip after processing one frame (default: 0)",
    )
    parser.add_argument(
        "-s",
        "--sc8850",
        action="store_true",
        help="Whether to enable SC-8850 mode, which enables higher resolution on compatible machines",
    )
    parser.add_argument(
        "-i",
        "--interlace",
        action="store_true",
        help="Whether to enable interlace mode, only useful when SC-8850 mode is enabled.",
    )
    parser.add_argument(
        "-e",
        "--sd90",
        action="store_true",
        help="Whether to enable reduced columns for SD-90, only useful when SC-8850 mode is enabled.",
    )

    args = parser.parse_args()

    process(args.input_video, args.frameskip, args.sc8850, args.interlace, args.sd90)


if __name__ == "__main__":
    main()
