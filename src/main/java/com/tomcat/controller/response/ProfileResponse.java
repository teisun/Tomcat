package com.tomcat.controller.response;

import com.tomcat.domain.UserProfile;
import lombok.AllArgsConstructor;
import lombok.Data;

@Data
@AllArgsConstructor
public class ProfileResponse {

    public ProfileResponse(){}

    private Long id;

    private Long userId;

    private String motherTongue;

    private String languageDepth;

    private String communicationStyle;

    private String targetLanguage;

    public static ProfileResponse build(UserProfile profile){
        ProfileResponse response = new ProfileResponse();
        response.setId(profile.getId());
        response.setUserId(profile.getUser().getId());
        response.setMotherTongue(profile.getMotherTongue());
        response.setLanguageDepth(profile.getLanguageDepth());
        response.setCommunicationStyle(profile.getCommunicationStyle());
        response.setTargetLanguage(profile.getTargetLanguage());
        return response;
    }

}
