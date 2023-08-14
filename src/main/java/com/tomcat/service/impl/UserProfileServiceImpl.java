package com.tomcat.service.impl;

import com.tomcat.controller.requeset.ProfileResquest;
import com.tomcat.controller.response.ProfileResponse;
import com.tomcat.domain.User;
import com.tomcat.domain.UserProfile;
import com.tomcat.domain.UserProfileRepository;
import com.tomcat.domain.UserRepository;
import com.tomcat.service.UserProfileService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.stereotype.Service;

@Service
public class UserProfileServiceImpl implements UserProfileService {
  
  @Autowired
  private UserProfileRepository userProfileRepository;

  @Autowired
  UserRepository userRepository;

  @Override
  public ProfileResponse getByUserId(String userId) {
    UserProfile userProfile = userProfileRepository.findByUserId(userId);
    ProfileResponse response;
    if(userProfile != null) response = ProfileResponse.build(userProfile);
    else {
      response = new ProfileResponse();
      response.setUserId(userId);

    }
    return response;
  }

  @Override
  public ProfileResponse update(ProfileResquest profileRequest) {

    User user = userRepository.findById(profileRequest.getUserId())
            .orElseThrow(() -> new RuntimeException("用户不存在"));

    UserProfile profile = new UserProfile();
    profile.setUser(user);
    profile.setMotherTongue(profileRequest.getMotherTongue());
    profile.setCommunicationStyle(profileRequest.getCommunicationStyle());
    profile.setLanguageDepth(profileRequest.getLanguageDepth());
    profile.setTargetLanguage(profileRequest.getTargetLanguage());

    UserProfile save = userProfileRepository.save(profile);
    ProfileResponse response = ProfileResponse.build(save);


    return response;
  }

}