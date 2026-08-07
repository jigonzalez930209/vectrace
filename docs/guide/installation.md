# Installation & Package Management

Vectrace is distributed in multiple formats to ensure compatibility with all major Linux distributions.

![Installation Options](/images/toolbar-preview.png)

## Package Formats

### 1. AppImage (Universal Package)
Recommended for all Linux distributions. No installation required:
```bash
chmod +x Vectrace-v0.1.0-x86_64.AppImage
./Vectrace-v0.1.0-x86_64.AppImage
```

### 2. Debian / Ubuntu (.deb)
```bash
sudo dpkg -i vectrace_0.1.0_amd64.deb
sudo apt-get install -f
```

### 3. Fedora / RHEL / openSUSE (.rpm)
```bash
sudo dnf install ./vectrace-0.1.0-1.x86_64.rpm
```

### 4. Arch Linux / Manjaro
```bash
cd packaging/arch
makepkg -si
```

---

## Extra System Packages & Prerequisites

When building from source or running standalone binaries, the following native packages are required:

| Distribution | Command to Install Dependencies |
| :--- | :--- |
| **Debian / Ubuntu / Mint** | `sudo apt-get install -y build-essential libx11-dev libxext-dev libxrender-dev libwayland-dev libdbus-1-dev` |
| **Fedora / RHEL / CentOS** | `sudo dnf install -y gcc libX11-devel libXext-devel libXrender-devel wayland-devel dbus-devel` |
| **Arch Linux / Manjaro** | `sudo pacman -S --needed base-devel libx11 libxext libxrender wayland dbus` |
| **openSUSE** | `sudo zypper install -t pattern devel_C_C++ && sudo zypper install libX11-devel libXext-devel wayland-devel dbus-1-devel` |
