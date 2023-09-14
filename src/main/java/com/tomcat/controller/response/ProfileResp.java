package com.tomcat.controller.response;

import com.tomcat.domain.UserProfile;
import lombok.AllArgsConstructor;
import lombok.Data;

@Data
@AllArgsConstructor
public class ProfileResp {

    public ProfileResp(){}

    private String id;

    private String userId;

    private String nativeLanguage;

    private String depth;

    private String style;

    private String targetLanguage;

    public static ProfileResp build(UserProfile profile){
        ProfileResp response = new ProfileResp();
        response.setId(profile.getId());
        response.setUserId(profile.getUser().getId());
        response.setNativeLanguage(profile.getNativeLanguage());
        response.setDepth(profile.getDepth());
        response.setStyle(profile.getStyle());
        response.setTargetLanguage(profile.getTargetLanguage());
        return response;
    }

    @Data
    public static class Config{
        private String nativeLanguage;

        private String depth;

        private String style;

        private String targetLanguage;
    }

    public Config buildConfig(){
        Config config = new Config();
        config.setDepth(this.depth);
        config.setStyle(this.style);
        config.setNativeLanguage(this.nativeLanguage);
        config.setTargetLanguage(this.targetLanguage);
        return config;
    }

}
