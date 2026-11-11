// buildSrc 独立构建的插件解析配置。
// plugins.gradle.org 的制品 CDN 在部分网络环境不可达（首页可达但 /m2/ 制品 404），
// 因此把阿里云 gradle-plugin 镜像放在最前，portal 作为回退。
pluginManagement {
    repositories {
        maven("https://maven.aliyun.com/repository/gradle-plugin")
        gradlePluginPortal()
        mavenCentral()
        google()
    }
}

dependencyResolutionManagement {
    repositories {
        google()
        mavenCentral()
    }
}
