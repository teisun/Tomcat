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

    private String motherTongue;

    private String languageDepth;

    private String communicationStyle;

    private String targetLanguage;

    public static ProfileResp build(UserProfile profile){
        ProfileResp response = new ProfileResp();
        response.setId(profile.getId());
        response.setUserId(profile.getUser().getId());
        response.setMotherTongue(profile.getMotherTongue());
        response.setLanguageDepth(profile.getLanguageDepth());
        response.setCommunicationStyle(profile.getCommunicationStyle());
        response.setTargetLanguage(profile.getTargetLanguage());
        return response;
    }

}
