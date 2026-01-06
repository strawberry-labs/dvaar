/** @type {import('next').NextConfig} */
const nextConfig = {
  images: {
    remotePatterns: [{ hostname: "localhost" }, { hostname: "randomuser.me" }],
  },
  transpilePackages: ["geist"],
  async redirects() {
    return [
      {
        source: "/install.sh",
        destination: "https://raw.githubusercontent.com/strawberry-labs/dvaar/main/scripts/install.sh",
        permanent: false,
      },
      {
        source: "/install.ps1",
        destination: "https://raw.githubusercontent.com/strawberry-labs/dvaar/main/scripts/install.ps1",
        permanent: false,
      },
    ];
  },
};

export default nextConfig;
